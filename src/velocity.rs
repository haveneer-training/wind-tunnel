//! Champs de vitesse porteurs.
//!
//! Le solveur ne connaît qu'un contrat — [`VelocityField`] — et se moque de savoir si
//! la vitesse vient d'une formule ou, plus tard, d'un calcul de potentiel. C'est le
//! rôle d'un trait : nommer ce dont on a besoin, pas ce dont on dispose.

use crate::geom::{Point, Vec2};
use crate::mesh::VertexId;

/// Un champ de vitesse stationnaire et incompressible.
///
/// `Sync` est exigé dès maintenant pour que le champ puisse être lu depuis plusieurs
/// threads à l'étape de parallélisation, sans avoir à modifier le contrat ensuite.
pub trait VelocityField: Sync {
    /// Vitesse au point donné.
    fn at(&self, p: Point) -> Vec2;

    /// Fonction de courant `ψ`, telle que `u = (∂ψ/∂y, −∂ψ/∂x)`.
    ///
    /// Elle existe dès que l'écoulement est à divergence nulle, ses lignes de niveau sont
    /// les lignes de courant, et sa différence entre deux points est le débit qui passe
    /// entre eux. `docs/etapes/etape-04.md` en donne la démonstration, convention de signe
    /// comprise — le `ψ(b) − ψ(a)` ci-dessous se déduit de l'orientation directe des
    /// cellules et de `Vec2::perp_cw`, il n'est pas une convention arbitraire.
    ///
    /// Elle n'est pas là par élégance mathématique. Le débit à travers une arête
    /// `a → b` vaut exactement `ψ(b) − ψ(a)` ; le solveur calcule donc ses débits
    /// par différences de `ψ` plutôt qu'en échantillonnant la vitesse. Le bilan de
    /// débit d'une cellule fermée devient alors une somme télescopique, nulle à la
    /// précision machine.
    ///
    /// L'enjeu est concret : avec une vitesse simplement échantillonnée aux faces, la
    /// divergence discrète est petite mais non nulle, et un schéma décentré — qui
    /// devrait être borné — fabrique du traceur. On l'a vu monter à 1,78 pour une
    /// donnée initiale valant au plus 1. Discrétiser le bon objet vaut mieux que
    /// rattraper l'erreur ensuite.
    fn stream(&self, p: Point) -> f64;
}

/// Ce dont le solveur a réellement besoin : `ψ` à chaque **sommet** du maillage.
///
/// [`VelocityField`] promet plus que ça — une vitesse et une fonction de courant en
/// *n'importe quel* point — et c'est une promesse qu'un écoulement calculé ne peut pas
/// tenir : il ne connaît `ψ` qu'aux sommets où il l'a résolue, et rien entre eux. D'où
/// ce second contrat, plus pauvre, qui est exactement celui que `compute_face_flux`
/// consomme : le débit d'une face ne demande jamais que les valeurs de ses deux
/// extrémités.
///
/// L'implémentation générale ci-dessous fait que **tout** [`VelocityField`] est déjà une
/// source de fonction de courant : les écoulements analytiques n'ont rien à écrire, et
/// l'étape 12 ajoute un type qui n'implémente que ce trait-ci.
pub trait StreamSource: Sync {
    /// Fonction de courant au sommet `id`, dont les coordonnées sont `p`.
    ///
    /// Les deux arguments sont redondants pour un écoulement analytique (qui n'utilise
    /// que `p`) comme pour un écoulement calculé (qui n'utilise que `id`) : c'est le
    /// prix à payer pour que le même appel serve aux deux.
    fn stream_at(&self, id: VertexId, p: Point) -> f64;
}

impl<F: VelocityField + ?Sized> StreamSource for F {
    fn stream_at(&self, _id: VertexId, p: Point) -> f64 {
        self.stream(p)
    }
}

/// Un écoulement uniforme.
#[derive(Clone, Copy, Debug)]
pub struct Uniform {
    /// La vitesse, partout la même.
    pub value: Vec2,
}

impl VelocityField for Uniform {
    fn at(&self, _p: Point) -> Vec2 {
        self.value
    }

    fn stream(&self, p: Point) -> f64 {
        self.value.x * p.y - self.value.y * p.x
    }
}

/// Écoulement potentiel autour d'un cylindre, avec circulation.
///
/// Solution classique de l'écoulement parfait incompressible irrotationnel : le
/// potentiel complexe `w(z) = U (z + R²/z) − iΓ/(2π) ln z` donne, en cartésien relatif
/// au centre,
///
/// ```text
/// u =  U (1 − R²(x² − y²)/r⁴) − Γ y / (2π r²)
/// v = −U (2 R² x y / r⁴)      + Γ x / (2π r²)
/// ```
///
/// La circulation `Γ` casse la symétrie haut/bas : c'est l'effet Magnus, et c'est ce
/// qui rend l'image intéressante à regarder.
#[derive(Clone, Copy, Debug)]
pub struct PotentialCylinder {
    /// Centre du cylindre.
    pub center: Point,
    /// Rayon du cylindre.
    pub radius: f64,
    /// Vitesse à l'infini, selon `x`.
    pub speed: f64,
    /// Circulation `Γ`.
    pub circulation: f64,
}

impl VelocityField for PotentialCylinder {
    #[cfg_attr(not(feature = "step4"), allow(unused_variables))] // trou étape 4
    fn at(&self, p: Point) -> Vec2 {
        // TODO-STEP:4 Évaluer l'écoulement potentiel autour du cylindre (formules ci-dessus),
        // en renvoyant une vitesse nulle très près du centre pour éviter la division par r⁴
        // SOLUTION-BEGIN
        let d = p - self.center;
        let r2 = d.dot(d);
        if r2 < 1e-12 * self.radius * self.radius {
            return Vec2::ZERO;
        }
        let r4 = r2 * r2;
        let (u, v) = (self.speed, self.circulation / (2.0 * std::f64::consts::PI));
        Vec2::new(
            u * (1.0 - self.radius * self.radius * (d.x * d.x - d.y * d.y) / r4) - v * d.y / r2,
            -u * (2.0 * self.radius * self.radius * d.x * d.y / r4) + v * d.x / r2,
        )
        // SOLUTION-END
    }

    fn stream(&self, p: Point) -> f64 {
        // ψ = U y (1 − R²/r²) − (Γ/2π) ln(r/R)
        let d = p - self.center;
        let r2 = d.dot(d);
        if r2 < 1e-12 * self.radius * self.radius {
            return 0.0;
        }
        self.speed * d.y * (1.0 - self.radius * self.radius / r2)
            - self.circulation / (2.0 * std::f64::consts::PI) * (r2.sqrt() / self.radius).ln()
    }
}

#[cfg(all(test, feature = "step4"))]
mod tests {
    use super::*;

    fn cylinder() -> PotentialCylinder {
        PotentialCylinder {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
            speed: 2.0,
            circulation: 0.0,
        }
    }

    #[test]
    fn uniform_velocity_far_from_the_obstacle() {
        let c = cylinder();
        let far = c.at(Point::new(1000.0, 0.0));
        assert!((far.x - c.speed).abs() < 1e-4);
        assert!(far.y.abs() < 1e-4);
    }

    #[test]
    fn flow_slips_along_the_cylinder_wall() {
        // sans circulation, la composante normale doit être nulle sur r = R
        let c = cylinder();
        for k in 0..16 {
            let theta = k as f64 * std::f64::consts::TAU / 16.0;
            let p = Point::new(c.radius * theta.cos(), c.radius * theta.sin());
            let n = Vec2::new(theta.cos(), theta.sin());
            assert!(
                c.at(p).dot(n).abs() < 1e-12,
                "composante normale non nulle en θ = {theta}"
            );
        }
    }

    #[test]
    fn stream_function_matches_the_velocity() {
        let mut c = cylinder();
        c.circulation = 3.0;
        let h = 1e-6;
        for p in [
            Point::new(2.0, 1.3),
            Point::new(-3.0, 0.7),
            Point::new(0.5, 2.5),
        ] {
            let dpsi_dy = (c.stream(Point::new(p.x, p.y + h)) - c.stream(Point::new(p.x, p.y - h)))
                / (2.0 * h);
            let dpsi_dx = (c.stream(Point::new(p.x + h, p.y)) - c.stream(Point::new(p.x - h, p.y)))
                / (2.0 * h);
            let u = c.at(p);
            assert!((u.x - dpsi_dy).abs() < 1e-5, "u_x ≠ ∂ψ/∂y en {p:?}");
            assert!((u.y + dpsi_dx).abs() < 1e-5, "u_y ≠ −∂ψ/∂x en {p:?}");
        }
    }

    #[test]
    fn circulation_breaks_the_symmetry() {
        let mut c = cylinder();
        c.circulation = 4.0;
        let above = c.at(Point::new(0.0, 1.5));
        let below = c.at(Point::new(0.0, -1.5));
        // circulation positive (sens direct) : l'écoulement est freiné au-dessus,
        // accéléré en dessous — la portance change de côté avec le signe de Γ
        assert!(
            below.x > above.x + 0.1,
            "above = {above:?}, below = {below:?}"
        );
    }
}
