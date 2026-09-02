//! Point d'entrée : lit un masque, engendre le maillage, advecte un rideau de fumée
//! et écrit une série d'images.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use wind_tunnel::error::SolverError;
use wind_tunnel::field::Field;
use wind_tunnel::flux::{Centered, FluxScheme, Upwind};
use wind_tunnel::geom::{Point, Vec2};
use wind_tunnel::io::{png, vtk};
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver};
use wind_tunnel::velocity::{PotentialCylinder, Uniform, VelocityField};

const USAGE: &str = "\
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
  --circulation <m2/s> circulation autour du cylindre  (défaut : 0)
  --diffusivity <m2/s> diffusivité du traceur          (défaut : 0)
  --dt <s>             pas de temps imposé             (défaut : déduit de la CFL)
  --scheme <nom>       upwind | centered               (défaut : upwind)
";

struct Args {
    mask: PathBuf,
    out: PathBuf,
    steps: usize,
    every: usize,
    width: u32,
    h: f64,
    refine: usize,
    bands: usize,
    speed: f64,
    circulation: f64,
    diffusivity: f64,
    dt: Option<f64>,
    scheme: String,
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
            circulation: 0.0,
            diffusivity: 0.0,
            dt: None,
            scheme: "upwind".to_string(),
        }
    }
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);
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
fn obstacle(mask: &Mask, h: f64) -> Option<(Point, f64)> {
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

fn run() -> Result<(), Box<dyn Error>> {
    let Some(args) = parse_args().map_err(|e| format!("{e}\n\n{USAGE}"))? else {
        print!("{USAGE}");
        return Ok(());
    };

    let mask = Mask::from_file(&args.mask)?.refine(args.refine.max(1));
    // Raffiner subdivise les cases : on réduit d'autant le pas pour que le domaine
    // physique, lui, ne bouge pas.
    let h = args.h / args.refine.max(1) as f64;
    let mesh = Mesh::from_mask(&mask, h)?;
    let (tri, quad) = mesh.shape_counts();
    let (xmin, ymin, xmax, ymax) = mesh.bounds();

    let velocity: Box<dyn VelocityField> = match obstacle(&mask, h) {
        Some((center, radius)) => Box::new(PotentialCylinder {
            center,
            radius,
            speed: args.speed,
            circulation: args.circulation,
        }),
        None => Box::new(Uniform {
            value: Vec2::new(args.speed, 0.0),
        }),
    };

    let config = Config {
        diffusivity: args.diffusivity,
        dt: args.dt,
        steps: args.steps,
        output_every: args.every,
        ..Config::default()
    };
    let scheme: Box<dyn FluxScheme> = match args.scheme.as_str() {
        "upwind" => Box::new(Upwind),
        "centered" => Box::new(Centered),
        other => return Err(format!("schéma inconnu : {other} (upwind ou centered)").into()),
    };
    let solver = Solver::new(&mesh, velocity.as_ref(), scheme, config)?;

    // Rideau de fumée initial : des bandes horizontales, comme le peigne de fumigènes
    // d'une soufflerie. Leur déformation autour de l'obstacle est tout le spectacle,
    // et leur étalement progressif est la diffusion numérique du schéma décentré.
    // n bandes de fumée séparées par n−1 intervalles vides, soit 2n−1 bandes en tout.
    let stripe = (ymax - ymin) / (2 * args.bands.max(1) - 1) as f64;
    let mut c = Field::from_fn(&mesh, |id| {
        let y = mesh.cell(id).centroid.y;
        let band = ((y - ymin) / stripe).floor() as i64;
        if band % 2 == 0 {
            1.0
        } else {
            0.0
        }
    });

    std::fs::create_dir_all(&args.out)?;
    println!(
        "maillage   : {} cellules ({tri} triangles, {quad} quadrangles), {} faces, {} sommets",
        mesh.n_cells(),
        mesh.n_faces(),
        mesh.n_vertices()
    );
    println!(
        "domaine    : [{xmin:.2}, {xmax:.2}] × [{ymin:.2}, {ymax:.2}], aire {:.2}",
        mesh.total_area()
    );
    println!(
        "pas de temps : {:.4e} s (maximum stable {:.4e} s)",
        solver.dt(),
        solver.max_stable_dt()
    );
    println!("sortie     : {}", args.out.display());

    let initial_mass = c.total_mass(&mesh);
    let out = args.out.clone();
    let every = args.every.max(1);
    solver.run(&mut c, |step, time, field| {
        let frame = step / every;
        vtk::write_vtk(
            out.join(format!("frame_{frame:04}.vtk")),
            &mesh,
            &[("c", field)],
        )
        .map_err(SolverError::Output)?;
        png::write_png(
            out.join(format!("frame_{frame:04}.png")),
            &mesh,
            field,
            args.width,
            (0.0, 1.0),
        )
        .map_err(|e| SolverError::Output(std::io::Error::other(e)))?;
        let (lo, hi) = field.min_max();
        println!(
            "  pas {step:5}  t = {time:8.3}  c ∈ [{lo:.3}, {hi:.3}]  masse {:.6}",
            field.total_mass(&mesh)
        );
        Ok(())
    })?;

    println!(
        "masse initiale {initial_mass:.6}, finale {:.6} (la fumée sort par l'aval)",
        c.total_mass(&mesh)
    );
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
