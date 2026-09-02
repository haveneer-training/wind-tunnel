//! Schémas de flux convectif.
//!
//! Un schéma répond à une seule question : quelle valeur du traceur transporter à
//! travers une face, sachant l'état des deux côtés et le sens de l'écoulement ?

use crate::geom::Vec2;

/// L'état vu par un schéma sur une face donnée.
///
/// Les valeurs sont orientées : `c_left` est la cellule dont la normale est sortante,
/// et `un` la vitesse normale correspondante. Un schéma n'a donc jamais à savoir
/// dans quel sens la face a été construite.
///
/// Les champs de gradient, depuis l'étape 7, portent de quoi reconstruire un ordre 2 :
/// voir [`crate::gradient`]. Un schéma qui n'en a pas besoin — `Upwind`, `Centered` —
/// les ignore simplement.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaceState {
    /// Valeur dans la cellule amont de la face au sens de la normale.
    pub c_left: f64,
    /// Valeur de l'autre côté (cellule voisine ou état de bord).
    pub c_right: f64,
    /// Vitesse normale `u·n`, sortante du côté gauche.
    pub un: f64,
    /// Gradient limité de la cellule gauche.
    pub grad_left: Vec2,
    /// Déplacement du centroïde gauche jusqu'au milieu de la face.
    pub to_face_left: Vec2,
    /// Gradient limité de la cellule droite, si le voisin est intérieur. `None` sur un
    /// bord : la valeur qui s'y trouve est déjà celle à retenir, rien à extrapoler.
    pub grad_right: Option<Vec2>,
    /// Déplacement du centroïde droit jusqu'au milieu de la face, si voisin intérieur.
    pub to_face_right: Vec2,
}

/// Un schéma de flux convectif.
pub trait FluxScheme: Sync {
    /// Valeur du traceur à retenir sur la face.
    fn interface_value(&self, s: &FaceState) -> f64;
}

/// Permet de choisir un schéma à l'exécution plutôt qu'à la compilation.
///
/// L'appel passe alors par une table virtuelle, au lieu d'être intégré à l'appelant.
/// La différence de coût se mesure — c'est l'objet d'un banc d'essai à l'étape 7.
impl FluxScheme for Box<dyn FluxScheme> {
    fn interface_value(&self, s: &FaceState) -> f64 {
        (**self).interface_value(s)
    }
}

/// Schéma décentré amont : on prend la valeur du côté d'où vient le fluide.
///
/// Inconditionnellement stable sous CFL et jamais oscillant, mais diffusif : un front
/// net s'étale à mesure qu'il traverse le domaine. C'est précisément ce défaut que la
/// reconstruction d'ordre 2 viendra corriger.
#[derive(Clone, Copy, Debug, Default)]
pub struct Upwind;

impl FluxScheme for Upwind {
    #[cfg_attr(not(feature = "step5"), allow(unused_variables))] // trou étape 5
    fn interface_value(&self, s: &FaceState) -> f64 {
        // TODO-STEP:5 Retenir la valeur du côté d'où vient le fluide, selon le signe de `un`
        // SOLUTION-BEGIN
        if s.un >= 0.0 {
            s.c_left
        } else {
            s.c_right
        }
        // SOLUTION-END
    }
}

/// Schéma centré : moyenne des deux côtés.
///
/// Présent pour la comparaison : d'ordre 2 mais oscillant en advection dominante,
/// il montre qu'« ordre plus élevé » et « meilleur » sont deux choses différentes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Centered;

impl FluxScheme for Centered {
    fn interface_value(&self, s: &FaceState) -> f64 {
        0.5 * (s.c_left + s.c_right)
    }
}

/// Schéma décentré d'ordre 2 : reconstruction MUSCL à partir du gradient limité de la
/// seule cellule amont.
///
/// Contrairement à `Centered`, qui moyenne les deux côtés, on n'extrapole que la
/// cellule d'où vient le fluide — c'est ce qui garde le schéma décentré, donc borné.
/// Le gradient utilisé est déjà limité (voir [`crate::gradient::limited_gradients`]) :
/// sans cette précaution, la reconstruction suffirait à faire déborder le champ de
/// `[0, 1]`, comme `Centered` (étape 6).
#[derive(Clone, Copy, Debug, Default)]
pub struct Muscl;

impl FluxScheme for Muscl {
    #[cfg_attr(not(feature = "step7"), allow(unused_variables))] // trou étape 7
    fn interface_value(&self, s: &FaceState) -> f64 {
        // TODO-STEP:7 Extrapoler linéairement la valeur amont jusqu'à la face à
        // partir de son gradient limité ; sans voisin intérieur de ce côté, retenir la
        // valeur de bord telle quelle
        // SOLUTION-BEGIN
        if s.un >= 0.0 {
            s.c_left + s.grad_left.dot(s.to_face_left)
        } else {
            match s.grad_right {
                Some(grad) => s.c_right + grad.dot(s.to_face_right),
                None => s.c_right,
            }
        }
        // SOLUTION-END
    }
}

#[cfg(all(test, feature = "step5"))]
mod tests {
    use super::*;

    #[test]
    fn upwind_follows_the_flow_direction() {
        let s = FaceState {
            c_left: 1.0,
            c_right: 2.0,
            un: 3.0,
            ..Default::default()
        };
        assert_eq!(Upwind.interface_value(&s), 1.0);
        assert_eq!(Upwind.interface_value(&FaceState { un: -3.0, ..s }), 2.0);
    }
}

#[cfg(all(test, feature = "step7"))]
mod muscl_tests {
    use super::*;

    #[test]
    fn muscl_extrapolates_the_upwind_side() {
        let s = FaceState {
            c_left: 1.0,
            c_right: 2.0,
            un: 3.0,
            grad_left: Vec2::new(0.5, 0.0),
            to_face_left: Vec2::new(0.2, 0.0),
            grad_right: Some(Vec2::new(-1.0, 0.0)),
            to_face_right: Vec2::new(-0.2, 0.0),
        };
        assert!((Muscl.interface_value(&s) - 1.1).abs() < 1e-12);

        let reversed = FaceState { un: -3.0, ..s };
        assert!((Muscl.interface_value(&reversed) - 2.2).abs() < 1e-12);
    }

    #[test]
    fn muscl_falls_back_to_the_boundary_value_without_an_inner_neighbour() {
        let s = FaceState {
            c_left: 1.0,
            c_right: 0.0,
            un: -3.0,
            grad_right: None,
            ..Default::default()
        };
        assert_eq!(Muscl.interface_value(&s), 0.0);
    }
}
