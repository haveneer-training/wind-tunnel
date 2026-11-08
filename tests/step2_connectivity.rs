#![cfg(feature = "step2")]

//! Un domaine en plusieurs morceaux est refusé dès la lecture du masque : le calcul
//! n'aurait aucun sens. C'est la première erreur que construit le stagiaire.

use wind_tunnel::error::MeshError;
use wind_tunnel::mask::Mask;

#[test]
fn a_domain_split_in_two() {
    // une cloison qui traverse toute la veine : le calcul n'aurait aucun sens
    match Mask::parse("...\n###\n...\n") {
        Err(MeshError::Disconnected { components }) => assert_eq!(components, 2),
        other => panic!("attendu Disconnected, obtenu {other:?}"),
    }
}
