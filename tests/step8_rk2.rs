#![cfg(feature = "step8")]

//! Le même cas qu'à l'étape 6 (`tests/step6_order.rs`), avec la reconstruction MUSCL de
//! l'étape 7 et le RK2 de l'étape 8 : l'ordre mesuré doit enfin passer à 2.

mod common;

use wind_tunnel::flux::Muscl;
use wind_tunnel::solver::TimeScheme;

#[test]
fn order_of_convergence_reaches_two_with_muscl_and_rk2() {
    let order = common::observed_order(Muscl, TimeScheme::Rk2);
    assert!(
        (order - 2.0).abs() < 0.4,
        "ordre observé {order:.2} (attendu ≈ 2 : reconstruction MUSCL + RK2, tous deux \
         d'ordre 2 — c'est ce qui résout l'énigme des étapes 6-7)"
    );
}
