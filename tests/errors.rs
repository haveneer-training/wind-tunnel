//! Chaque façon de se tromper doit produire *son* erreur, avec de quoi la corriger.
//!
//! Ces tests ne vérifient pas que le programme « ne plante pas » : ils vérifient qu'il
//! dit précisément ce qui ne va pas et où. C'est la différence entre un code qu'on
//! donne à quelqu'un d'autre et un code qu'on garde pour soi.

use wind_tunnel::error::{MeshError, SolverError};
use wind_tunnel::flux::Upwind;
use wind_tunnel::geom::Vec2;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver};
use wind_tunnel::velocity::Uniform;

#[test]
fn inconsistent_line_length() {
    match Mask::parse("...\n..\n") {
        Err(MeshError::RaggedMask {
            line,
            expected,
            got,
        }) => {
            assert_eq!((line, expected, got), (2, 3, 2));
        }
        other => panic!("attendu RaggedMask, obtenu {other:?}"),
    }
}

#[test]
fn unknown_character_is_located() {
    match Mask::parse("..\n.x\n") {
        Err(MeshError::InvalidChar { line, col, ch }) => {
            assert_eq!((line, col, ch), (2, 2, 'x'));
        }
        other => panic!("attendu InvalidChar, obtenu {other:?}"),
    }
}

#[test]
fn a_fully_solid_domain() {
    assert!(matches!(
        Mask::parse("###\n###\n"),
        Err(MeshError::EmptyDomain)
    ));
}

#[test]
fn a_domain_split_in_two() {
    // une cloison qui traverse toute la veine : le calcul n'aurait aucun sens
    match Mask::parse("...\n###\n...\n") {
        Err(MeshError::Disconnected { components }) => assert_eq!(components, 2),
        other => panic!("attendu Disconnected, obtenu {other:?}"),
    }
}

#[test]
fn the_error_message_points_at_the_fault() {
    let err = Mask::parse("..\n.@\n").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("ligne 2"), "message : {message}");
    assert!(message.contains("colonne 2"), "message : {message}");
}

#[test]
fn unstable_time_step_is_rejected_before_computing() {
    let mask = Mask::parse("....\n....\n").unwrap();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();
    let flow = Uniform {
        value: Vec2::new(1.0, 0.0),
    };
    let config = Config {
        dt: Some(42.0),
        ..Config::default()
    };
    match Solver::new(&mesh, &flow, Upwind, config) {
        Err(SolverError::Cfl { dt, dt_max }) => {
            assert_eq!(dt, 42.0);
            assert!(dt_max < dt, "dt_max = {dt_max}");
        }
        Err(other) => panic!("attendu Cfl, obtenu {other}"),
        Ok(_) => panic!("le solveur a accepté un pas de temps instable"),
    }
}

#[test]
fn a_missing_file() {
    let err = Mask::from_file("domains/ce-fichier-n-existe-pas.dom").unwrap_err();
    assert!(matches!(err, MeshError::Io(_)), "{err}");
    // la cause d'origine reste accessible, comme le veut `std::error::Error`
    assert!(std::error::Error::source(&err).is_some());
}
