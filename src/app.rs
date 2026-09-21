//! Montage d'un cas de calcul, partagé par les deux exécutables.
//!
//! Le binaire séquentiel (`src/main.rs`) et le binaire MPI (`mpi/src/main.rs`, étape 11)
//! lisent les mêmes options et construisent le même cas ; seul diffère ce sur quoi ils le
//! construisent — le domaine entier pour l'un, une bande pour l'autre. Tout ce qui leur
//! est commun vit donc ici plutôt que d'être écrit deux fois.

use std::path::PathBuf;

use clap::Parser;

use crate::field::Field;
use crate::flux::{Centered, FluxScheme, Muscl, Upwind};
use crate::geom::{Point, Vec2};
use crate::mask::Mask;
use crate::mesh::{BoundaryKind, Mesh};
use crate::solver::{Bc, Config, TimeScheme};
#[cfg(feature = "step12")]
use crate::stream::{ComputedStream, StreamOptions};
use crate::velocity::{PotentialCylinder, StreamSource, Uniform, VelocityField};

// Décrites une seule fois, ici, via les attributs `clap` : nom de l'option, valeur par
// défaut et texte d'aide vivent côte à côte sur le champ qu'ils concernent, plutôt que
// dupliqués entre une structure et un désassemblage d'argv écrit à la main. `--help`,
// les valeurs par défaut affichées et les erreurs sur une valeur invalide (`--refine
// abc`, une option manquante, une option inconnue) en découlent tous, gratuitement.
/// Les options de la ligne de commande, une fois analysées.
#[derive(Parser, Clone, Debug)]
#[command(
    name = "wind-tunnel",
    about = "Soufflerie numérique",
    long_about = None,
    after_help = "Sous MPI (wind-tunnel-mpi) : --flow computed et --every-dt sont \
                  indisponibles — aucun rang ne détient le maillage complet, et la \
                  boucle en temps y est distincte."
)]
pub struct Args {
    /// Fichier de masque du domaine.
    pub mask: PathBuf,

    /// Répertoire de sortie ; ouvrir le frames.vtk.series qui s'y trouve, pas les
    /// frame_*.vtk.
    #[arg(long, default_value = "out", help_heading = "Sorties")]
    pub out: PathBuf,

    /// Plafond en pas de temps (défaut : 600, sauf si --max-time est donné — auquel cas
    /// il n'y a pas de plafond en pas).
    #[arg(long, help_heading = "Durée du calcul")]
    pub max_steps: Option<usize>,

    /// Période de sortie, en pas de temps.
    #[arg(long, default_value_t = 10, help_heading = "Sorties")]
    pub every: usize,

    /// Largeur des images PNG, en pixels.
    #[arg(long, default_value_t = 900, help_heading = "Sorties")]
    pub width: u32,

    /// Côté d'une cellule, en m, avant raffinement ; change la taille physique du
    /// domaine, pas le nombre de cellules.
    #[arg(long = "cell-size", default_value_t = 1.0, help_heading = "Maillage")]
    pub h: f64,

    /// Subdivise chaque case du masque en k×k ; seul moyen d'augmenter la résolution, le
    /// masque fixant le nombre de cases.
    #[arg(long, default_value_t = 1, help_heading = "Maillage")]
    pub refine: usize,

    /// Nombre de bandes de fumée du rideau initial.
    #[arg(long, default_value_t = 4, help_heading = "Transport du traceur")]
    pub bands: usize,

    /// Vitesse de l'écoulement à l'infini, en m/s.
    #[arg(long, default_value_t = 1.0, help_heading = "Écoulement porteur")]
    pub speed: f64,

    /// Inclinaison de l'écoulement uniforme, en degrés ; sans obstacle et sans --flow
    /// computed uniquement.
    #[arg(long, default_value_t = 0.0, help_heading = "Écoulement porteur")]
    pub angle: f64,

    /// Circulation autour du cylindre, en m²/s ; écoulement analytique seul.
    #[arg(long, default_value_t = 0.0, help_heading = "Écoulement porteur")]
    pub circulation: f64,

    /// Diffusivité du traceur, en m²/s.
    #[arg(long, default_value_t = 0.0, help_heading = "Transport du traceur")]
    pub diffusivity: f64,

    /// Pas de temps imposé, en s ; déduit de la CFL si absent.
    #[arg(long, help_heading = "Transport du traceur")]
    pub dt: Option<f64>,

    /// Schéma de flux : upwind, centered ou muscl.
    #[arg(long, default_value = "upwind", help_heading = "Transport du traceur")]
    pub scheme: String,

    /// Schéma en temps : euler ou rk2.
    #[arg(long, default_value = "euler", help_heading = "Transport du traceur")]
    pub time_scheme: String,

    /// Écoulement porteur : analytic ou computed (« computed » résout ∇²ψ = 0 sur le
    /// maillage — étape 12 — et respecte alors la forme dessinée et les parois).
    #[arg(long, default_value = "analytic", help_heading = "Écoulement porteur")]
    pub flow: String,

    /// Résidu visé par la résolution de la fonction de courant (--flow computed).
    #[arg(long, default_value_t = 1e-6, help_heading = "Écoulement porteur")]
    pub stream_tol: f64,

    /// Nombre maximal de balayages de la résolution de la fonction de courant, idem.
    #[arg(long, default_value_t = 200_000, help_heading = "Écoulement porteur")]
    pub stream_iters: usize,

    /// Période de sortie en temps physique, en s ; remplace --every si elle est donnée,
    /// et donne aux images des dates comparables d'un calcul à l'autre.
    #[arg(long, help_heading = "Sorties")]
    pub every_dt: Option<f64>,

    /// Durée simulée, en s ; remplace le plafond en pas si elle est donnée — c'est la
    /// durée, pas le nombre de pas, qui se compare d'un calcul à l'autre.
    #[arg(long, help_heading = "Durée du calcul")]
    pub max_time: Option<f64>,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            mask: PathBuf::new(),
            out: PathBuf::from("out"),
            max_steps: None,
            every: 10,
            width: 900,
            h: 1.0,
            refine: 1,
            bands: 4,
            speed: 1.0,
            angle: 0.0,
            circulation: 0.0,
            diffusivity: 0.0,
            dt: None,
            scheme: "upwind".to_string(),
            time_scheme: "euler".to_string(),
            flow: "analytic".to_string(),
            stream_tol: 1e-6,
            stream_iters: 200_000,
            every_dt: None,
            max_time: None,
        }
    }
}

impl Args {
    /// Côté effectif d'une cellule, une fois le raffinement appliqué.
    ///
    /// Raffiner subdivise les cases : on réduit d'autant le pas pour que le domaine
    /// physique, lui, ne bouge pas.
    pub fn cell_size(&self) -> f64 {
        self.h / self.refine.max(1) as f64
    }

    /// Masque du domaine, lu puis raffiné.
    pub fn read_mask(&self) -> Result<Mask, crate::error::MeshError> {
        Ok(Mask::from_file(&self.mask)?.refine(self.refine.max(1)))
    }
}

/// Aide en ligne, commune aux deux exécutables — le rendu `clap` de [`Args`].
pub fn help_text() -> String {
    <Args as clap::CommandFactory>::command()
        .render_long_help()
        .to_string()
}

/// Analyse les arguments du processus ; `Ok(None)` signifie « afficher l'aide ».
pub fn parse_args() -> Result<Option<Args>, String> {
    parse_from(std::env::args().skip(1))
}

/// Analyse une suite d'arguments quelconque — c'est ce que teste la suite de tests.
///
/// `argv` ne porte pas le nom du programme (comme `std::env::args().skip(1)`) : `clap`
/// en attend un en tête de son entrée, `parse_args` le lui fournit donc ici plutôt que
/// d'imposer aux appelants de le fabriquer eux-mêmes.
pub fn parse_from(argv: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let argv = std::iter::once("wind-tunnel".to_string()).chain(argv);
    match Args::try_parse_from(argv) {
        Ok(args) => Ok(Some(args)),
        Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Centre et rayon équivalents de l'obstacle dessiné dans le masque, s'il y en a un.
///
/// À l'étape 11, chaque rang ne maille que sa bande mais appelle cette fonction sur le
/// masque **global** : sinon chaque rang inventerait un obstacle différent, et donc un
/// écoulement différent.
pub fn obstacle(mask: &Mask, h: f64) -> Option<(Point, f64)> {
    let mut count = 0.0;
    let (mut sx, mut sy) = (0.0, 0.0);
    for row in 0..mask.rows() {
        for col in 0..mask.cols() {
            if mask.is_fluid(row, col) {
                continue;
            }
            count += 1.0;
            sx += (col as f64 + 0.5) * h;
            sy += (mask.rows() - row) as f64 * h - 0.5 * h;
        }
    }
    if count == 0.0 {
        return None;
    }
    // rayon du disque de même aire que l'obstacle dessiné
    let radius = (count * h * h / std::f64::consts::PI).sqrt();
    Some((Point::new(sx / count, sy / count), radius))
}

/// L'écoulement porteur déduit du masque et des options.
pub fn velocity_for(mask: &Mask, args: &Args, h: f64) -> Result<Box<dyn VelocityField>, String> {
    match obstacle(mask, h) {
        Some(_) if args.angle != 0.0 => Err(format!(
            "--angle {} n'a de sens que sur un masque sans obstacle : avec un obstacle, \
             l'écoulement est celui du cylindre.\nEssayez domains/tunnel-empty.dom.",
            args.angle
        )),
        Some((center, radius)) => Ok(Box::new(PotentialCylinder {
            center,
            radius,
            speed: args.speed,
            circulation: args.circulation,
        })),
        None => {
            // Sans obstacle, l'écoulement est uniforme et son inclinaison devient
            // réglable : c'est ce qui permet d'isoler la fausse diffusion du schéma,
            // qui ne dépend que de l'angle entre l'écoulement et les axes du maillage.
            let theta = args.angle.to_radians();
            Ok(Box::new(Uniform {
                value: Vec2::new(args.speed * theta.cos(), args.speed * theta.sin()),
            }))
        }
    }
}

/// L'écoulement porteur du cas, quelle que soit son origine.
///
/// Les deux variantes ne satisfont pas le même contrat — l'analytique répond en tout
/// point, le calculé seulement aux sommets du maillage — mais toutes deux savent donner
/// `ψ` là où le solveur la lit. C'est ce que dit [`Carrier::as_source`].
pub enum Carrier {
    /// Une formule : écoulement uniforme ou potentiel autour d'un cylindre.
    Analytic(Box<dyn VelocityField>),
    /// Une fonction de courant résolue sur le maillage (étape 12).
    #[cfg(feature = "step12")]
    Computed(ComputedStream),
}

// `Carrier` est lui-même une source de fonction de courant : c'est ce qui permet de le
// passer tel quel à `Solver::new`. Un `&dyn VelocityField` ne peut pas être converti en
// `&dyn StreamSource` — on ne réétiquette pas un objet-trait déjà dénué de taille — donc
// l'aiguillage se fait ici, sur l'énumération, plutôt que par coercition.
impl StreamSource for Carrier {
    // `id` ne sert qu'à la variante calculée : l'avertissement vient du `#[cfg]`, pas
    // d'un trou, et il n'a donc rien à dire au stagiaire.
    #[cfg_attr(not(feature = "step12"), allow(unused_variables))] // collatéral du cfg étape 12
    fn stream_at(&self, id: crate::mesh::VertexId, p: Point) -> f64 {
        match self {
            Carrier::Analytic(flow) => flow.stream(p),
            #[cfg(feature = "step12")]
            Carrier::Computed(stream) => stream.stream_at(id, p),
        }
    }
}

/// L'écoulement porteur demandé par `--flow`, monté sur le maillage déjà construit.
#[cfg_attr(not(feature = "step12"), allow(unused_variables))] // collatéral du cfg étape 12
pub fn carrier_for(
    mask: &Mask,
    mesh: &Mesh,
    args: &Args,
    h: f64,
) -> Result<(Carrier, Option<(usize, f64)>), String> {
    match args.flow.as_str() {
        "analytic" => Ok((Carrier::Analytic(velocity_for(mask, args, h)?), None)),
        #[cfg(feature = "step12")]
        "computed" => {
            if args.angle != 0.0 {
                return Err(format!(
                    "--angle {} n'a pas de sens avec --flow computed : l'écoulement \
                     calculé entre horizontalement dans la veine et suit ensuite la \
                     géométrie.",
                    args.angle
                ));
            }
            if args.circulation != 0.0 {
                return Err("--circulation ne s'applique qu'à l'écoulement analytique \
                            autour du cylindre ; avec --flow computed, la circulation \
                            autour de l'obstacle est celle que fixe le calcul."
                    .to_string());
            }
            let options = StreamOptions {
                speed: args.speed,
                max_iters: args.stream_iters,
                tol: args.stream_tol,
            };
            let (stream, iters, residual) = ComputedStream::solve(mesh, &options).map_err(|e| {
                format!(
                    "{e}\nLe balayage de Jacobi converge lentement : relevez \
                     --stream-iters, relâchez --stream-tol, ou maillez plus grossièrement \
                     (--refine)."
                )
            })?;
            Ok((Carrier::Computed(stream), Some((iters, residual))))
        }
        #[cfg(not(feature = "step12"))]
        "computed" => Err("--flow computed arrive à l'étape 12".to_string()),
        other => Err(format!(
            "écoulement inconnu : {other} (analytic ou computed)"
        )),
    }
}

/// Le schéma de flux nommé par `--scheme`.
pub fn flux_scheme(args: &Args) -> Result<Box<dyn FluxScheme>, String> {
    match args.scheme.as_str() {
        "upwind" => Ok(Box::new(Upwind)),
        "centered" => Ok(Box::new(Centered)),
        "muscl" => Ok(Box::new(Muscl)),
        other => Err(format!(
            "schéma inconnu : {other} (upwind, centered ou muscl)"
        )),
    }
}

/// Les deux plafonds d'arrêt, une fois la règle des valeurs par défaut appliquée.
///
/// Le piège qu'elle évite : `--max-time 60` seul s'arrêterait en réalité au bout de 600
/// pas, le défaut de l'autre plafond l'emportant en silence — et 600 pas ne font pas 60
/// secondes. **Un défaut ne doit pas contraindre une valeur explicitement demandée.** Donc
/// un plafond donné seul est le seul qui compte, et les 600 pas ne servent que lorsqu'on
/// n'a rien demandé du tout.
pub fn caps(args: &Args) -> (usize, Option<f64>) {
    match (args.max_steps, args.max_time) {
        (Some(steps), time) => (steps, time),
        (None, Some(time)) => (usize::MAX, Some(time)),
        (None, None) => (DEFAULT_MAX_STEPS, None),
    }
}

/// Nombre de pas par défaut, quand ni `--max-steps` ni `--max-time` n'est donné.
const DEFAULT_MAX_STEPS: usize = 600;

/// La configuration du solveur déduite des options.
pub fn solver_config(args: &Args) -> Result<Config, String> {
    let time_scheme = match args.time_scheme.as_str() {
        "euler" => TimeScheme::Euler,
        "rk2" => TimeScheme::Rk2,
        other => return Err(format!("schéma temporel inconnu : {other} (euler ou rk2)")),
    };
    let mut config = Config {
        diffusivity: args.diffusivity,
        dt: args.dt,
        max_steps: caps(args).0,
        output_every: args.every,
        output_dt: args.every_dt,
        max_time: caps(args).1,
        time_scheme,
        ..Config::default()
    };

    // Les parois d'un écoulement calculé sont des lignes de courant exactes : aucun débit
    // ne les traverse, et le flux nul cesse d'être un mensonge commode (voir
    // `Config::default` et `src/stream.rs`).
    if args.flow == "computed" {
        config.bc.insert(BoundaryKind::Wall, Bc::NoFlux);
        config.bc.insert(BoundaryKind::Obstacle, Bc::NoFlux);
    }
    Ok(config)
}

/// Rideau de fumée initial : des bandes horizontales, comme le peigne de fumigènes
/// d'une soufflerie.
///
/// Leur déformation autour de l'obstacle est tout le spectacle, et leur étalement
/// progressif est la diffusion numérique du schéma décentré. `bands` bandes de fumée
/// séparées par `bands − 1` intervalles vides, soit `2·bands − 1` bandes en tout.
///
/// Les bandes sont **horizontales** et les sous-domaines de l'étape 11 **verticaux** :
/// `ymin` et `ymax` sont donc les mêmes pour tous les rangs, et cette fonction donne le
/// bon champ sur une bande sans rien savoir de la décomposition.
pub fn initial_field(mesh: &Mesh, bands: usize) -> Field {
    let (_, ymin, _, ymax) = mesh.bounds();
    let stripe = (ymax - ymin) / (2 * bands.max(1) - 1) as f64;
    Field::from_fn(mesh, |id| {
        let y = mesh.cell(id).centroid.y;
        let band = ((y - ymin) / stripe).floor() as i64;
        if band % 2 == 0 {
            1.0
        } else {
            0.0
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_cap_never_constrains_a_requested_one() {
        let steps_only = Args {
            max_steps: Some(50),
            ..Args::default()
        };
        assert_eq!(caps(&steps_only), (50, None));

        // Le cas qui motive toute cette fonction : sans elle, `--max-time 60` s'arrêtait
        // au bout de 600 pas, qui ne font pas 60 secondes.
        let time_only = Args {
            max_time: Some(60.0),
            ..Args::default()
        };
        assert_eq!(caps(&time_only), (usize::MAX, Some(60.0)));

        let both = Args {
            max_steps: Some(50),
            max_time: Some(60.0),
            ..Args::default()
        };
        assert_eq!(caps(&both), (50, Some(60.0)));

        assert_eq!(caps(&Args::default()), (DEFAULT_MAX_STEPS, None));
    }

    #[test]
    fn the_command_line_fills_both_caps() {
        let parse = |argv: &[&str]| {
            parse_from(argv.iter().map(|s| s.to_string()))
                .expect("analyse impossible")
                .expect("aide demandée")
        };
        assert_eq!(caps(&parse(&["m.dom", "--max-time", "60"])).0, usize::MAX);
        assert_eq!(caps(&parse(&["m.dom", "--max-steps", "50"])), (50, None));
    }
}
