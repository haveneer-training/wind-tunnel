//! Soufflerie numérique distribuée : le même calcul, sur plusieurs processus.
//!
//! ```shell
//! mpirun -n 4 wind-tunnel-mpi domains/tunnel.dom --refine 4 --steps 200 --out out-mpi
//! ```
//!
//! Chaque rang ne construit un maillage que sur **sa** bande du domaine : personne ne
//! détient la soufflerie entière, et c'est tout l'intérêt. Le reste du projet est
//! réutilisé tel quel — maillage, champ, schémas, solveur, sorties — parce que le
//! solveur travaille déjà sur « un maillage », sans jamais supposer que c'est tout le
//! domaine.

mod exchange;

use std::error::Error;
use std::process::ExitCode;

use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use wind_tunnel::app::{self, USAGE};
use wind_tunnel::decomposition::{owned_mass, owned_min_max, Bands, Layout, HALO};
use wind_tunnel::field::Field;
use wind_tunnel::flux::FluxScheme;
use wind_tunnel::io::{png, vtk};
use wind_tunnel::solver::{Config, Solver, TimeScheme};

use exchange::{global_dt_max, global_min_max, global_sum, Halo};

/// Ce que le pilote fait du champ à intervalle régulier : l'écrire et le diagnostiquer.
///
/// Le `Halo` traverse la signature plutôt que d'être capturé : il n'en existe qu'un par
/// rang, la boucle en temps s'en sert entre deux appels au rapporteur, et le compilateur
/// refuserait deux emprunts mutables simultanés. Le passer en argument, c'est le prêter
/// à tour de rôle.
type Reporter<'a> = dyn FnMut(usize, &mut Field, &mut Halo) -> Result<(), Box<dyn Error>> + 'a;

/// Déroule le calcul distribué, du premier pas au dernier.
///
/// C'est [`Solver::run`] réécrit ici, et pour une seule raison : il faut reprendre la
/// main entre les évaluations de résidu pour communiquer. La boucle en temps appartient
/// désormais au pilote.
// Huit paramètres, un de plus que ce que tolère `clippy` : les regrouper dans une
// structure « pilote » n'apporterait rien ici, où chacun a un rôle distinct et une
// durée de vie différente. On assume, en le disant.
#[allow(clippy::too_many_arguments)]
fn time_loop<F: FluxScheme>(
    world: &SimpleCommunicator,
    layout: &Layout,
    solver: &Solver<'_, F>,
    config: &Config,
    every: usize,
    c: &mut Field,
    halo: &mut Halo,
    report: &mut Reporter<'_>,
) -> Result<(), Box<dyn Error>> {
    // TODO-STEP:11 Écrire la boucle en temps : `halo.exchange` avant CHAQUE évaluation
    // de résidu — une fois par pas en Euler, deux fois en RK2, car le prédicteur a lui
    // aussi des cellules fantômes à rafraîchir — puis `solver.residual` et la mise à
    // jour du champ, et `report(step, c, halo)` tous les `every` pas.
    // SOLUTION-BEGIN
    let dt = solver.dt();
    // Les trois tampons sont alloués avant la boucle, pas dedans : `predictor` et `k2`
    // ne servent qu'en RK2, mais ils ne coûtent qu'un `Vec<f64>` chacun, une fois pour
    // tout le calcul. Voir `docs/BONUS-OPTIMISATION.md`.
    let mut k1 = Field::zeros(c.len());
    let mut k2 = Field::zeros(c.len());
    let mut predictor = Field::zeros(c.len());

    for step in 1..=config.max_steps {
        halo.exchange(world, layout, c);
        solver.residual(c, &mut k1);

        match config.time_scheme {
            TimeScheme::Euler => {
                for (value, rate) in c.as_mut_slice().iter_mut().zip(k1.as_slice()) {
                    *value += dt * rate;
                }
            }
            TimeScheme::Rk2 => {
                // `copy_from` recopie dans un tampon existant, là où `c.clone()`
                // allouerait un champ neuf à chaque pas de temps.
                predictor.copy_from(c);
                for (value, rate) in predictor.as_mut_slice().iter_mut().zip(k1.as_slice()) {
                    *value += dt * rate;
                }
                // Le prédicteur est un champ comme un autre : ses cellules fantômes
                // sont périmées tant qu'on ne les a pas redemandées aux voisins.
                halo.exchange(world, layout, &mut predictor);
                solver.residual(&predictor, &mut k2);
                for ((value, a), b) in c
                    .as_mut_slice()
                    .iter_mut()
                    .zip(k1.as_slice())
                    .zip(k2.as_slice())
                {
                    *value += 0.5 * dt * (a + b);
                }
            }
        }

        if every > 0 && step % every == 0 {
            report(step, c, halo)?;
        }
    }
    Ok(())
    // SOLUTION-END
}

fn run() -> Result<(), Box<dyn Error>> {
    let universe = mpi::initialize().ok_or("MPI n'a pas pu être initialisé")?;
    let world = universe.world();
    let rank = world.rank() as usize;
    let parts = world.size() as usize;
    let root = rank == 0;

    let Some(args) = app::parse_args().map_err(|e| format!("{e}\n\n{USAGE}"))? else {
        if root {
            print!("{USAGE}");
        }
        return Ok(());
    };

    // Le masque est un fichier texte de quelques kilo-octets : chaque rang le lit, et
    // c'est la dernière chose qu'ils aient tous en entier. L'obstacle — donc
    // l'écoulement porteur — s'en déduit sur le domaine *global*, sinon chaque rang
    // inventerait son propre obstacle et son propre écoulement.
    let mask = args.read_mask()?;
    let h = args.cell_size();
    // L'écoulement calculé de l'étape 12 résout `ψ` sur *un* maillage, et aucun rang ne
    // détient le maillage complet : le faire ici demanderait soit que chaque rang maille
    // le domaine entier — ce que toute l'étape 11 s'emploie à éviter — soit un gradient
    // conjugué distribué, avec échange de halo à chaque produit matrice-vecteur. C'est un
    // bon exercice, et c'est celui que propose `docs/etapes/etape-12.md` ; en attendant,
    // mieux vaut le dire que faire semblant.
    // La cadence en temps physique est mise en œuvre par `Solver::run`, que ce pilote
    // n'utilise pas : il a sa propre boucle, celle qui échange les halos, et c'est
    // l'exercice de l'étape 11. Plutôt que d'alourdir ce trou, on refuse l'option.
    if args.every_dt.is_some() {
        return Err(
            "--every-dt n'est pas disponible sous MPI : le pilote distribué a sa \
                    propre boucle en temps, qui sort tous les --every pas.\nUtilisez \
                    --every, ou l'exécutable séquentiel."
                .into(),
        );
    }
    if args.flow != "analytic" {
        return Err(format!(
            "--flow {} n'est pas disponible sous MPI : l'écoulement calculé demande un \
             maillage complet, qu'aucun rang ne possède.\nUtilisez l'exécutable \
             séquentiel, ou --flow analytic.",
            args.flow
        )
        .into());
    }
    let velocity = app::velocity_for(&mask, &args, h)?;

    let bands = Bands::new(mask.cols(), parts, HALO)?;
    let layout = Layout::new(&mask, &bands, rank);
    let mesh = layout.build_mesh(&mask, h)?;
    let mut config = app::solver_config(&args)?;

    // Le pas de temps doit être le même partout : on prend le plus petit de tous.
    let probe = Solver::new(
        &mesh,
        velocity.as_ref(),
        app::flux_scheme(&args)?,
        config.clone(),
    )?;
    let dt_max = global_dt_max(&world, probe.max_stable_dt());
    config.dt = Some(args.dt.unwrap_or(config.cfl * dt_max));
    // `--max-time` s'exprime en secondes, la boucle distribuée compte en pas : la
    // conversion se fait ici, une fois le pas de temps connu — et donne le même nombre sur
    // tous les rangs, puisque `dt_max` vient d'une réduction globale. Les deux plafonds se
    // ramènent alors à un seul, le plus petit, et la boucle n'a rien à savoir.
    if let Some(end) = config.max_time.filter(|end| *end > 0.0) {
        let dt = config.dt.expect("pas de temps fixé juste au-dessus");
        config.max_steps = config.max_steps.min((end / dt).ceil() as usize);
    }
    drop(probe);

    let solver = Solver::new(
        &mesh,
        velocity.as_ref(),
        app::flux_scheme(&args)?,
        config.clone(),
    )?;
    let mut c = app::initial_field(&mesh, args.bands);

    std::fs::create_dir_all(&args.out)?;
    if root {
        println!("rangs      : {parts}, halo de {HALO} colonnes");
        println!(
            "pas de temps : {:.4e} s (maximum stable {dt_max:.4e} s, réduit sur tous les rangs)",
            solver.dt()
        );
        println!("sortie     : {}", args.out.display());
    }
    let cells = global_sum(&world, layout.owned.len() as f64) as usize;
    println!(
        "  rang {rank:3} : colonnes {:?}, {} cellules possédées, {} fantômes",
        bands.owned(rank),
        layout.owned.len(),
        mesh.n_cells() - layout.owned.len()
    );
    if root {
        println!("maillage   : {cells} cellules au total, réparties sur {parts} rangs");
    }

    let every = args.every.max(1);
    let initial_mass = global_sum(&world, owned_mass(&mesh, &c, &layout));

    // `--width` est la largeur de l'image du domaine entier : celle d'une bande en est
    // la fraction correspondante, pour que toutes les bandes soient à la même échelle
    // et se recollent au pixel près.
    let band_width = (args.width as usize * layout.extended.len() / mask.cols()).max(1) as u32;

    let report = |step: usize, c: &mut Field, halo: &mut Halo| -> Result<(), Box<dyn Error>> {
        // Les cellules fantômes datent de l'échange qui a précédé le dernier résidu :
        // un pas de retard, visible sur les images comme une couture. Un échange de
        // plus, et les bandes se recollent exactement.
        halo.exchange(&world, &layout, c);
        let frame = step / every;
        // Pas de rassemblement : chaque rang écrit sa bande. Rapatrier le champ sur le
        // rang 0 l'obligerait à mailler tout le domaine — exactement ce qu'on a passé
        // l'étape à éviter. Les fichiers portent des coordonnées globales grâce à la
        // translation faite par `Layout::build_mesh`, donc les bandes se recollent au
        // pixel près. Le rang précède le pas de temps dans le nom (`rankN_frame_XXXX`,
        // pas l'inverse) : ParaView détecte une série temporelle sur le seul nombre qui
        // touche l'extension, donc avec `rank` en tête et `frame` juste avant `.vtk` il
        // ouvre une série par rang, chacune animée sur le bon axe — pas une par pas de
        // temps avec les rangs mélangés dedans. (Un `.pvd` explicite serait plus propre
        // mais fait planter le lecteur `vtkPVDReader` de ParaView 6.1.1 sur du VTK
        // legacy — testé, pas une supposition.)
        vtk::write_vtk(
            args.out.join(format!("rank{rank}_frame_{frame:04}.vtk")),
            &mesh,
            &[("c", c)],
        )?;
        png::write_png(
            args.out.join(format!("rank{rank}_frame_{frame:04}.png")),
            &mesh,
            c,
            band_width,
            (0.0, 1.0),
        )
        .map_err(|e| format!("écriture PNG : {e}"))?;

        // Les diagnostics sont des réductions sur les cellules *possédées* : une
        // cellule fantôme appartient à quelqu'un d'autre, la compter ici la compterait
        // deux fois.
        let mass = global_sum(&world, owned_mass(&mesh, c, &layout));
        let (lo, hi) = global_min_max(&world, owned_min_max(c, &layout));
        if root {
            println!(
                "  pas {step:5}  t = {:8.3}  c ∈ [{lo:.3}, {hi:.3}]  masse {mass:.6}",
                step as f64 * solver.dt()
            );
        }
        Ok(())
    };

    // Un seul jeu de tampons d'échange pour tout le calcul : leur taille ne dépend que
    // du découpage, qui ne bouge plus.
    let mut halo = Halo::new(&layout);
    report(0, &mut c, &mut halo)?;

    let mut report = report;
    time_loop(
        &world,
        &layout,
        &solver,
        &config,
        every,
        &mut c,
        &mut halo,
        &mut report,
    )?;

    let final_mass = global_sum(&world, owned_mass(&mesh, &c, &layout));
    if root {
        println!(
            "masse : {initial_mass:.6} → {final_mass:.6} ({:+.1} %)",
            100.0 * (final_mass - initial_mass) / initial_mass
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur : {e}");
            let mut source = e.source();
            while let Some(cause) = source {
                eprintln!("  cause : {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}
