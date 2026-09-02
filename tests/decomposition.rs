#![cfg(feature = "step11")]

//! Le découpage en bandes de l'étape 11 se vérifie sans MPI : ce qui est délicat n'est
//! pas de faire circuler des octets, c'est de savoir qui possède quoi et dans quel ordre.
//!
//! Le dernier test est le plus important : il rejoue la décomposition dans un seul
//! processus, échange les halos par mémoire, et compare au calcul monolithique. Si
//! celui-là passe, il ne reste au crate `mpi/` qu'à transporter les mêmes tampons.

use wind_tunnel::decomposition::{owned_mass, pack, unpack, Bands, Layout, HALO};
use wind_tunnel::field::Field;
use wind_tunnel::flux::Muscl;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver, TimeScheme};
use wind_tunnel::velocity::{Uniform, VelocityField};

/// Le domaine livré : c'est le seul à avoir des coins rentrants, donc des chanfreins,
/// donc des cellules dont la géométrie dépend de leurs voisines. Un obstacle
/// rectangulaire n'en produit aucun et ne testerait rien de la largeur du halo.
fn mask() -> Mask {
    Mask::from_file("domains/tunnel.dom").unwrap()
}

#[test]
fn bands_cover_every_column_exactly_once() {
    for (cols, parts) in [(12, 3), (10, 3), (10, 4), (7, 1), (9, 3), (100, 7)] {
        let bands = Bands::new(cols, parts, 1).unwrap();
        let mut next = 0;
        for part in 0..parts {
            let owned = bands.owned(part);
            assert_eq!(
                owned.start, next,
                "bande {part} de {cols} colonnes en {parts}"
            );
            assert!(owned.end > owned.start, "bande {part} vide");
            next = owned.end;
        }
        assert_eq!(next, cols, "les bandes doivent couvrir tout le masque");
    }
}

#[test]
fn bands_narrower_than_the_halo_are_refused() {
    assert!(
        Bands::new(10, 5, 3).is_err(),
        "bandes de 2 colonnes, halo 3"
    );
    assert!(Bands::new(10, 1, 3).is_ok(), "un seul rang n'a pas de halo");
}

#[test]
fn extended_bands_stay_inside_the_domain() {
    let bands = Bands::new(24, 3, HALO).unwrap();
    assert_eq!(bands.extended(0).start, 0, "pas de halo hors du domaine");
    assert_eq!(bands.extended(2).end, 24);
    assert_eq!(
        bands.extended(1),
        bands.owned(1).start - HALO..bands.owned(1).end + HALO
    );
}

#[test]
fn halo_send_recv_lists_match() {
    let mask = mask();
    let bands = Bands::new(mask.cols(), 3, HALO).unwrap();
    let layouts: Vec<Layout> = (0..3).map(|p| Layout::new(&mask, &bands, p)).collect();
    let globals: Vec<Vec<_>> = layouts.iter().map(|l| l.global_cells(&mask)).collect();

    for part in 0..2 {
        let (left, right) = (&layouts[part], &layouts[part + 1]);
        assert!(!left.send_right.is_empty(), "coupure {part} vide");
        // Les deux rangs doivent désigner les mêmes cellules du domaine, dans le même
        // ordre : c'est ce qui permet de n'échanger que des valeurs.
        let sent: Vec<_> = left
            .send_right
            .iter()
            .map(|&c| globals[part][c.index()])
            .collect();
        let received: Vec<_> = right
            .recv_left
            .iter()
            .map(|&c| globals[part + 1][c.index()])
            .collect();
        assert_eq!(sent, received, "coupure {part}, sens gauche → droite");

        let sent: Vec<_> = right
            .send_left
            .iter()
            .map(|&c| globals[part + 1][c.index()])
            .collect();
        let received: Vec<_> = left
            .recv_right
            .iter()
            .map(|&c| globals[part][c.index()])
            .collect();
        assert_eq!(sent, received, "coupure {part}, sens droite → gauche");
    }

    assert!(
        layouts[0].send_left.is_empty(),
        "le rang 0 n'a pas de voisin à gauche"
    );
    assert!(layouts[0].recv_left.is_empty());
    assert!(
        layouts[2].send_right.is_empty(),
        "le dernier rang n'a pas de voisin à droite"
    );
    assert!(layouts[2].recv_right.is_empty());
}

#[test]
fn every_cell_of_the_domain_is_owned_by_exactly_one_rank() {
    let mask = mask();
    let parts = 4;
    let bands = Bands::new(mask.cols(), parts, HALO).unwrap();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();

    let mut owner = vec![usize::MAX; mesh.n_cells()];
    for part in 0..parts {
        let layout = Layout::new(&mask, &bands, part);
        let globals = layout.global_cells(&mask);
        for &local in &layout.owned {
            let global = globals[local.index()];
            assert_eq!(
                owner[global.index()],
                usize::MAX,
                "cellule possédée deux fois"
            );
            owner[global.index()] = part;
        }
    }
    assert!(
        owner.iter().all(|&o| o != usize::MAX),
        "cellule sans propriétaire"
    );
}

#[test]
fn the_local_mesh_sits_where_the_band_does() {
    let mask = mask();
    let bands = Bands::new(mask.cols(), 3, HALO).unwrap();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();

    for part in 0..3 {
        let layout = Layout::new(&mask, &bands, part);
        let local = layout.build_mesh(&mask, 1.0).unwrap();
        let globals = layout.global_cells(&mask);
        assert_eq!(local.n_cells(), globals.len(), "rang {part}");

        // Chaque cellule possédée tombe sur la cellule globale de même centroïde :
        // c'est la translation qui remet la bande à sa place.
        //
        // L'énoncé ne vaut que pour les cellules possédées — et c'est la raison d'être
        // de la troisième colonne de halo. Une cellule de la colonne fantôme la plus
        // externe ne voit pas ce qu'il y a au-delà de la bande : si un solide s'y
        // trouve, elle est chanfreinée dans le maillage global et ne l'est pas dans le
        // maillage local, donc son centroïde diffère. Cela ne remonte jamais jusqu'aux
        // cellules possédées tant que le halo compte [`HALO`] colonnes.
        for &id in &layout.owned {
            let here = local.cell(id).centroid;
            let there = mesh.cell(globals[id.index()]).centroid;
            assert!(
                (here - there).norm() < 1e-12,
                "rang {part}, cellule {id:?} : {here:?} ≠ {there:?}"
            );
        }
    }
}

#[test]
fn the_initial_field_does_not_depend_on_the_band() {
    let mask = mask();
    let bands = Bands::new(mask.cols(), 3, HALO).unwrap();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();
    let whole = wind_tunnel::app::initial_field(&mesh, 3);

    for part in 0..3 {
        let layout = Layout::new(&mask, &bands, part);
        let local = layout.build_mesh(&mask, 1.0).unwrap();
        let globals = layout.global_cells(&mask);
        let band = wind_tunnel::app::initial_field(&local, 3);
        for &id in &layout.owned {
            assert_eq!(
                band[id],
                whole[globals[id.index()]],
                "rang {part}, cellule {id:?} : les bandes de fumée sont horizontales, \
                 elles ne doivent pas dépendre du découpage vertical"
            );
        }
    }
}

#[test]
fn owned_masses_sum_to_the_whole() {
    let mask = mask();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();
    let whole = wind_tunnel::app::initial_field(&mesh, 3);

    for parts in [1, 2, 3, 5] {
        let bands = Bands::new(mask.cols(), parts, HALO).unwrap();
        let mut total = 0.0;
        for part in 0..parts {
            let layout = Layout::new(&mask, &bands, part);
            let local = layout.build_mesh(&mask, 1.0).unwrap();
            let field = wind_tunnel::app::initial_field(&local, 3);
            total += owned_mass(&local, &field, &layout);
        }
        // Sommer le champ *entier* de chaque rang compterait les cellules fantômes
        // deux fois : la réduction ne doit porter que sur les cellules possédées.
        assert!(
            (total - whole.total_mass(&mesh)).abs() < 1e-9,
            "{parts} bandes : masse {total} au lieu de {}",
            whole.total_mass(&mesh)
        );
    }
}

/// Rejoue la décomposition dans un seul processus : c'est l'algorithme du crate `mpi/`,
/// avec des recopies mémoire à la place des messages.
fn run_decomposed(mask: &Mask, h: f64, parts: usize, steps: usize, dt: f64) -> Vec<(u32, f64)> {
    let bands = Bands::new(mask.cols(), parts, HALO).unwrap();
    let flow = Uniform {
        value: wind_tunnel::geom::Vec2::new(1.0, 0.4),
    };
    let config = Config {
        dt: Some(dt),
        steps,
        output_every: 0,
        time_scheme: TimeScheme::Rk2,
        ..Config::default()
    };

    let layouts: Vec<Layout> = (0..parts).map(|p| Layout::new(mask, &bands, p)).collect();
    let meshes: Vec<Mesh> = layouts
        .iter()
        .map(|l| l.build_mesh(mask, h).unwrap())
        .collect();
    let solvers: Vec<Solver<'_, Muscl>> = meshes
        .iter()
        .map(|m| Solver::new(m, &flow as &dyn VelocityField, Muscl, config.clone()).unwrap())
        .collect();
    let mut fields: Vec<Field> = meshes
        .iter()
        .map(|m| wind_tunnel::app::initial_field(m, 3))
        .collect();

    // L'échange que le crate `mpi/` fera avec `immediate_send` / `immediate_receive`.
    let exchange = |fields: &mut Vec<Field>| {
        for part in 0..parts.saturating_sub(1) {
            let going_right = pack(&fields[part], &layouts[part].send_right);
            unpack(
                &mut fields[part + 1],
                &layouts[part + 1].recv_left,
                &going_right,
            );
            let going_left = pack(&fields[part + 1], &layouts[part + 1].send_left);
            unpack(&mut fields[part], &layouts[part].recv_right, &going_left);
        }
    };

    let mut k1: Vec<Field> = meshes.iter().map(|m| Field::zeros(m.n_cells())).collect();
    let mut k2: Vec<Field> = meshes.iter().map(|m| Field::zeros(m.n_cells())).collect();

    for _ in 0..steps {
        // RK2 avec un échange avant *chaque* évaluation de résidu.
        exchange(&mut fields);
        let mut predictors = Vec::with_capacity(parts);
        for part in 0..parts {
            solvers[part].residual(&fields[part], &mut k1[part]);
            let mut predictor = fields[part].clone();
            for (value, rate) in predictor.as_mut_slice().iter_mut().zip(k1[part].as_slice()) {
                *value += dt * rate;
            }
            predictors.push(predictor);
        }
        exchange(&mut predictors);
        for part in 0..parts {
            solvers[part].residual(&predictors[part], &mut k2[part]);
            for ((value, a), b) in fields[part]
                .as_mut_slice()
                .iter_mut()
                .zip(k1[part].as_slice())
                .zip(k2[part].as_slice())
            {
                *value += 0.5 * dt * (a + b);
            }
        }
    }

    layouts
        .iter()
        .zip(&fields)
        .flat_map(|(l, f)| {
            let globals = l.global_cells(mask);
            l.owned
                .iter()
                .map(|&c| (globals[c.index()].0, f[c]))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn decomposed_run_matches_monolithic() {
    let mask = mask();
    let h = 1.0;
    let steps = 20;

    let mesh = Mesh::from_mask(&mask, h).unwrap();
    let flow = Uniform {
        value: wind_tunnel::geom::Vec2::new(1.0, 0.4),
    };
    let probe = Solver::new(&mesh, &flow as &dyn VelocityField, Muscl, Config::default()).unwrap();
    let dt = 0.4 * probe.max_stable_dt();

    let config = Config {
        dt: Some(dt),
        steps,
        output_every: 0,
        time_scheme: TimeScheme::Rk2,
        ..Config::default()
    };
    let solver = Solver::new(&mesh, &flow as &dyn VelocityField, Muscl, config).unwrap();
    let mut reference = wind_tunnel::app::initial_field(&mesh, 3);
    solver.run(&mut reference, |_, _, _| Ok(())).unwrap();

    for parts in [1, 2, 3, 4, 5, 7] {
        let mut worst = 0.0f64;
        for (global, value) in run_decomposed(&mask, h, parts, steps, dt) {
            worst = worst.max((value - reference.as_slice()[global as usize]).abs());
        }
        // Le calcul décomposé n'est pas *tenu* d'être identique au bit près — l'ordre
        // des faces peut différer entre maillage local et maillage global — mais il
        // doit rester au niveau du bruit d'arrondi. Un halo trop étroit se voit ici
        // tout de suite : à deux colonnes, l'écart monte à 8·10⁻³ sur trois rangs.
        assert!(
            worst < 1e-12,
            "{parts} bandes : écart maximal {worst:e} avec le calcul monolithique"
        );
    }
}
