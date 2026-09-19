//! Le même système que l'étape 12, résolu par un crate d'algèbre linéaire.
//!
//! `cargo run --release --features faer --example stream_faer`
//!
//! Le socle de l'étape 12 résout `∇²ψ = 0` par balayages de Jacobi : trente lignes, aucune
//! dépendance, et une convergence qui se dégrade comme le carré du raffinement. Un solveur
//! direct creux fait le même travail sans tolérance ni itérations — encore faut-il assembler
//! la matrice, ce que la version *matrix-free* n'avait jamais besoin de faire.
//!
//! Ce que cet exemple montre, au fond : ce n'est pas le solveur qui coûte, c'est le format.

use std::time::Instant;

use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use faer::Side;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::{BoundaryKind, Mesh};
use wind_tunnel::stream::{ComputedStream, StreamOptions};

const H: f64 = 1.0;

fn main() {
    let mask = Mask::from_file("domains/square.dom").expect("domains/square.dom illisible");
    let mask = mask.refine(2);
    let mesh = Mesh::from_mask(&mask, H).expect("maillage impossible");
    let options = StreamOptions::default();
    println!(
        "maillage : {} cellules, {} sommets",
        mesh.n_cells(),
        mesh.n_vertices()
    );

    // ---- la même chose que `boundary_values`, réécrite ici pour que l'exemple tienne seul
    let (_, ymin, _, _) = mesh.bounds();
    let vertices = mesh.vertices();
    let psi0 = |i: usize| options.speed * (vertices[i].y - ymin);
    let mut fixed: Vec<Option<f64>> = vec![None; mesh.n_vertices()];
    let groups = mesh.groups();
    for kind in [BoundaryKind::Inlet, BoundaryKind::Wall] {
        for id in groups.get(&kind).into_iter().flatten() {
            let face = mesh.face(*id);
            for v in [face.a, face.b] {
                fixed[v.index()] = Some(psi0(v.index()));
            }
        }
    }
    if let Some(faces) = groups.get(&BoundaryKind::Obstacle) {
        let mut sum = 0.0;
        let mut count = 0.0;
        for id in faces {
            let face = mesh.face(*id);
            for v in [face.a, face.b] {
                sum += vertices[v.index()].y;
                count += 1.0;
            }
        }
        let value = options.speed * (sum / count - ymin);
        for id in faces {
            let face = mesh.face(*id);
            for v in [face.a, face.b] {
                fixed[v.index()] = Some(value);
            }
        }
    }

    // ---- numérotation des seules inconnues : les sommets libres
    let mut index = vec![usize::MAX; mesh.n_vertices()];
    let mut free = Vec::new();
    for (i, value) in fixed.iter().enumerate() {
        if value.is_none() {
            index[i] = free.len();
            free.push(i);
        }
    }
    let n = free.len();

    // ---- assemblage en triplets : diagonale `Σ w`, extra-diagonale `−w`, et les valeurs
    // imposées qui passent au second membre. C'est exactement le travail que le produit
    // matrice-vecteur de l'étape 12 faisait sans jamais l'écrire.
    let start = Instant::now();
    let mut triplets: Vec<Triplet<usize, usize, f64>> = Vec::new();
    let mut diagonal = vec![0.0; n];
    let mut rhs = vec![0.0; n];
    for face in mesh.faces() {
        if face.is_boundary() {
            continue;
        }
        let w = face.distance / face.length;
        for (from, to) in [(face.a, face.b), (face.b, face.a)] {
            let (from, to) = (from.index(), to.index());
            if fixed[from].is_some() {
                continue;
            }
            let row = index[from];
            diagonal[row] += w;
            match fixed[to] {
                Some(value) => rhs[row] += w * value,
                None => triplets.push(Triplet::new(row, index[to], -w)),
            }
        }
    }
    for (row, d) in diagonal.iter().enumerate() {
        triplets.push(Triplet::new(row, row, *d));
    }
    let matrix =
        SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets).expect("assemblage");
    let assembly = start.elapsed();

    let start = Instant::now();
    let llt = matrix.sp_cholesky(Side::Lower).expect("Cholesky creuse");
    let solution = llt.solve(&faer::col::Col::from_fn(n, |i| rhs[i]));
    let direct = start.elapsed();

    let mut psi_direct = vec![0.0; mesh.n_vertices()];
    for (i, value) in fixed.iter().enumerate() {
        psi_direct[i] = value.unwrap_or_else(|| solution[index[i]]);
    }

    // ---- et le même problème par balayages de Jacobi, pour comparaison
    let start = Instant::now();
    let (jacobi, iters, residual) = ComputedStream::solve(&mesh, &options).expect("Jacobi");
    let iterative = start.elapsed();

    let worst = psi_direct
        .iter()
        .zip(jacobi.psi())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f64, f64::max);

    println!("inconnues libres : {n}");
    println!(
        "faer    : assemblage {:.3} s + Cholesky creuse {:.3} s = {:.3} s, solution exacte",
        assembly.as_secs_f64(),
        direct.as_secs_f64(),
        (assembly + direct).as_secs_f64()
    );
    println!(
        "Jacobi  : {:.3} s, {iters} balayages, résidu {residual:.2e}",
        iterative.as_secs_f64()
    );
    println!("écart maximal entre les deux : {worst:.3e}");
    println!(
        "\nL'écart n'est pas une erreur de faer : c'est ce qui manque encore à Jacobi pour\n\
         avoir convergé. Le résidu visé était {:.0e} ; le résidu n'est pas l'erreur.",
        options.tol
    );
}
