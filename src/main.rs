//! Point d'entrée : lit un masque, engendre le maillage, advecte un rideau de fumée
//! et écrit une série d'images.
//!
//! Tout le montage du cas — options, obstacle, écoulement, schémas, champ initial — vit
//! dans [`wind_tunnel::app`], partagé avec le binaire MPI de l'étape 11.

use std::error::Error;
use std::process::ExitCode;

use wind_tunnel::app::{self, USAGE};
use wind_tunnel::error::SolverError;
use wind_tunnel::io::{png, vtk};
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::Solver;

fn run() -> Result<(), Box<dyn Error>> {
    let Some(args) = app::parse_args().map_err(|e| format!("{e}\n\n{USAGE}"))? else {
        print!("{USAGE}");
        return Ok(());
    };

    let mask = args.read_mask()?;
    let h = args.cell_size();
    let mesh = Mesh::from_mask(&mask, h)?;
    let (tri, quad) = mesh.shape_counts();
    let (xmin, ymin, xmax, ymax) = mesh.bounds();

    let velocity = app::velocity_for(&mask, &args, h)?;
    let scheme = app::flux_scheme(&args)?;
    let config = app::solver_config(&args)?;
    let solver = Solver::new(&mesh, velocity.as_ref(), scheme, config)?;

    let mut c = app::initial_field(&mesh, args.bands);

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

    // On rapporte la variation sans l'interpréter : selon les conditions aux limites
    // et l'inclinaison de l'écoulement, le domaine peut aussi bien se vider que se
    // remplir — une paroi en gradient nul devenue frontière d'entrée réinjecte du
    // traceur, ce qui est un cas mal posé et se voit ici en clair.
    let final_mass = c.total_mass(&mesh);
    println!(
        "masse : {initial_mass:.6} → {final_mass:.6} ({:+.1} %)",
        100.0 * (final_mass - initial_mass) / initial_mass
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
