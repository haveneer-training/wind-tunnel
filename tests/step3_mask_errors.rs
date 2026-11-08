#![cfg(feature = "step3")]

//! Chaque façon de mal écrire un masque doit produire *son* erreur, avec de quoi la
//! corriger : la ligne et la colonne fautives dans le message, la cause d'origine
//! derrière une erreur d'entrée-sortie.
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

#[test]
fn the_error_message_points_at_the_fault() {
    let err = Mask::parse("..\n.@\n").unwrap_err();
    let message = err.to_string();
    assert!(message.contains("ligne 2"), "message : {message}");
    assert!(message.contains("colonne 2"), "message : {message}");
}

#[test]
fn a_missing_file() {
    let err = Mask::from_file("domains/ce-fichier-n-existe-pas.dom").unwrap_err();
    assert!(matches!(err, MeshError::Io(_)), "{err}");
    // la cause d'origine reste accessible, comme le veut `std::error::Error`
    assert!(std::error::Error::source(&err).is_some());
}
