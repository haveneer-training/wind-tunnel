#![cfg(feature = "step6")]

//! Vérifie l'ordre du schéma par solution manufacturée : un profil lisse, transporté à
//! vitesse uniforme, a une solution exacte connue — sa propre translation. L'écart entre
//! le champ numérique et cette translation doit décroître comme h¹ : c'est ce qu'on
//! attend d'un schéma décentré amont couplé à un Euler explicite, tous deux d'ordre 1.
//! (L'étape 7 relève l'ordre spatial à 2 ; l'ordre mesuré ici n'en bougera pourtant pas
//! avant l'étape 8 — l'erreur en temps domine dès que `dt ∝ h`.)
//!
//! Le cas lui-même, `advection_error`, est dans `tests/common/mod.rs` : l'étape 8 le
//! reprend.

mod common;

use wind_tunnel::flux::Upwind;
use wind_tunnel::solver::TimeScheme;

#[test]
fn order_of_convergence_is_about_one() {
    let order = common::observed_order(Upwind, TimeScheme::Euler);
    assert!(
        (order - 1.0).abs() < 0.3,
        "ordre observé {order:.2} (attendu ≈ 1 : décentrement amont + Euler explicite, \
         tous deux d'ordre 1)"
    );
}
