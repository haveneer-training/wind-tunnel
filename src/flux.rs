//! Schémas de flux convectif.
//!
//! Un schéma répond à une seule question : quelle valeur du traceur transporter à
//! travers une face, sachant l'état des deux côtés et le sens de l'écoulement ?

/// L'état vu par un schéma sur une face donnée.
///
/// Les valeurs sont orientées : `c_left` est la cellule dont la normale est sortante,
/// et `un` la vitesse normale correspondante. Un schéma n'a donc jamais à savoir
/// dans quel sens la face a été construite.
#[derive(Clone, Copy, Debug)]
pub struct FaceState {
    /// Valeur dans la cellule amont de la face au sens de la normale.
    pub c_left: f64,
    /// Valeur de l'autre côté (cellule voisine ou état de bord).
    pub c_right: f64,
    /// Vitesse normale `u·n`, sortante du côté gauche.
    pub un: f64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upwind_follows_the_flow_direction() {
        let s = FaceState {
            c_left: 1.0,
            c_right: 2.0,
            un: 3.0,
        };
        assert_eq!(Upwind.interface_value(&s), 1.0);
        assert_eq!(Upwind.interface_value(&FaceState { un: -3.0, ..s }), 2.0);
    }
}
