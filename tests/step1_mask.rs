#![cfg(feature = "step1")]

//! Chaque façon de mal écrire un masque doit produire *son* erreur, avec de quoi la
//! corriger.
//!
//! Ces tests ne vérifient pas que le programme « ne plante pas » : ils vérifient qu'il
//! dit précisément ce qui ne va pas et où. C'est la différence entre un code qu'on
//! donne à quelqu'un d'autre et un code qu'on garde pour soi.

use wind_tunnel::error::MeshError;
use wind_tunnel::mask::Mask;

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
