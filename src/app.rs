//! Montage d'un cas de calcul, partagé par les deux exécutables.
//!
//! Le binaire séquentiel (`src/main.rs`) et le binaire MPI (`mpi/src/main.rs`, étape 11)
//! lisent les mêmes options et construisent le même cas ; seul diffère ce sur quoi ils le
//! construisent — le domaine entier pour l'un, une bande pour l'autre. Tout ce qui leur
//! est commun vit donc ici plutôt que d'être écrit deux fois.

use std::path::PathBuf;

use crate::field::Field;
use crate::flux::{Centered, FluxScheme, Muscl, Upwind};
use crate::geom::{Point, Vec2};
use crate::mask::Mask;
use crate::mesh::{BoundaryKind, Mesh};
use crate::solver::{Bc, Config, TimeScheme};
#[cfg(feature = "step12")]
use crate::stream::{ComputedStream, StreamOptions};
use crate::velocity::{PotentialCylinder, StreamSource, Uniform, VelocityField};

/// Aide en ligne, commune aux deux exécutables.
pub const USAGE: &str = "\
Soufflerie numérique

  wind-tunnel <masque.dom> [options]

Options :
  --out <dir>          répertoire de sortie            (défaut : out)
  --steps <n>          nombre de pas de temps          (défaut : 600)
  --every <n>          période de sortie, en pas       (défaut : 10)
  --width <px>         largeur des images              (défaut : 900)
  --h <m>              côté d'une cellule              (défaut : 1)
  --refine <k>         subdivise chaque case en k×k    (défaut : 1)
  --bands <n>          nombre de bandes de fumée       (défaut : 4)
  --speed <m/s>        vitesse à l'infini              (défaut : 1)
  --angle <deg>        incline l'écoulement uniforme   (défaut : 0 ; sans effet
                       si le masque contient un obstacle)
  --circulation <m2/s> circulation autour du cylindre  (défaut : 0)
  --diffusivity <m2/s> diffusivité du traceur          (défaut : 0)
  --dt <s>             pas de temps imposé             (défaut : déduit de la CFL)
  --scheme <nom>       upwind | centered | muscl       (défaut : upwind)
  --time-scheme <nom>  euler | rk2                     (défaut : euler)
  --flow <nom>         analytic | computed             (défaut : analytic ; « computed »
                       résout l'écoulement sur le maillage, étape 12)
  --stream-tol <r>     résidu visé par --flow computed (défaut : 1e-6)
  --stream-iters <n>   balayages au plus               (défaut : 200000)
  --frame-dt <s>       sortir toutes les <s> secondes  (défaut : tous les --every pas ;
                       impose la date des images, donc rend deux calculs de pas de
                       temps différents comparables image par image)
";

/// Les options de la ligne de commande, une fois analysées.
#[derive(Clone, Debug)]
pub struct Args {
    /// Fichier de masque du domaine.
    pub mask: PathBuf,
    /// Répertoire où écrire les images et les fichiers VTK.
    pub out: PathBuf,
    /// Nombre de pas de temps.
    pub steps: usize,
    /// Période de sortie, en pas de temps.
    pub every: usize,
    /// Largeur des images produites, en pixels.
    pub width: u32,
    /// Côté d'une cellule du masque, avant raffinement.
    pub h: f64,
    /// Facteur de subdivision de chaque case du masque.
    pub refine: usize,
    /// Nombre de bandes de fumée du rideau initial.
    pub bands: usize,
    /// Vitesse de l'écoulement à l'infini.
    pub speed: f64,
    /// Inclinaison de l'écoulement uniforme, en degrés.
    pub angle: f64,
    /// Circulation autour du cylindre.
    pub circulation: f64,
    /// Diffusivité du traceur.
    pub diffusivity: f64,
    /// Pas de temps imposé ; déduit de la CFL s'il est absent.
    pub dt: Option<f64>,
    /// Nom du schéma de flux.
    pub scheme: String,
    /// Nom du schéma en temps.
    pub time_scheme: String,
    /// Nom de l'écoulement porteur : `analytic` ou `computed`.
    pub flow: String,
    /// Résidu visé par la résolution de la fonction de courant.
    pub stream_tol: f64,
    /// Nombre maximal de balayages de la résolution de la fonction de courant.
    pub stream_iters: usize,
    /// Période de sortie en temps physique ; remplace `every` si elle est donnée.
    pub frame_dt: Option<f64>,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            mask: PathBuf::new(),
            out: PathBuf::from("out"),
            steps: 600,
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
            frame_dt: None,
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

/// Analyse les arguments du processus ; `Ok(None)` signifie « afficher l'aide ».
pub fn parse_args() -> Result<Option<Args>, String> {
    parse_from(std::env::args().skip(1))
}

/// Analyse une suite d'arguments quelconque — c'est ce que teste la suite de tests.
pub fn parse_from(argv: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut args = Args::default();
    let mut argv = argv.into_iter();
    let mut mask_seen = false;

    while let Some(arg) = argv.next() {
        let mut value = || {
            argv.next()
                .ok_or_else(|| format!("l'option {arg} attend une valeur"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--out" => args.out = PathBuf::from(value()?),
            "--steps" => args.steps = value()?.parse().map_err(|e| format!("--steps : {e}"))?,
            "--every" => args.every = value()?.parse().map_err(|e| format!("--every : {e}"))?,
            "--width" => args.width = value()?.parse().map_err(|e| format!("--width : {e}"))?,
            "--h" => args.h = value()?.parse().map_err(|e| format!("--h : {e}"))?,
            "--refine" => args.refine = value()?.parse().map_err(|e| format!("--refine : {e}"))?,
            "--bands" => args.bands = value()?.parse().map_err(|e| format!("--bands : {e}"))?,
            "--speed" => args.speed = value()?.parse().map_err(|e| format!("--speed : {e}"))?,
            "--angle" => args.angle = value()?.parse().map_err(|e| format!("--angle : {e}"))?,
            "--circulation" => {
                args.circulation = value()?
                    .parse()
                    .map_err(|e| format!("--circulation : {e}"))?
            }
            "--diffusivity" => {
                args.diffusivity = value()?
                    .parse()
                    .map_err(|e| format!("--diffusivity : {e}"))?
            }
            "--dt" => args.dt = Some(value()?.parse().map_err(|e| format!("--dt : {e}"))?),
            "--scheme" => args.scheme = value()?,
            "--time-scheme" => args.time_scheme = value()?,
            "--flow" => args.flow = value()?,
            "--stream-tol" => {
                args.stream_tol = value()?
                    .parse()
                    .map_err(|e| format!("--stream-tol : {e}"))?
            }
            "--stream-iters" => {
                args.stream_iters = value()?
                    .parse()
                    .map_err(|e| format!("--stream-iters : {e}"))?
            }
            "--frame-dt" => {
                args.frame_dt = Some(value()?.parse().map_err(|e| format!("--frame-dt : {e}"))?)
            }
            other if other.starts_with('-') => return Err(format!("option inconnue : {other}")),
            other => {
                args.mask = PathBuf::from(other);
                mask_seen = true;
            }
        }
    }

    if !mask_seen {
        return Err("il manque le fichier de masque".to_string());
    }
    Ok(Some(args))
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
        steps: args.steps,
        output_every: args.every,
        output_dt: args.frame_dt,
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
