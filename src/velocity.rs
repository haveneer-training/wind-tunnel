//! Champs de vitesse porteurs.
//!
//! Le solveur ne connaît qu'un contrat — [`VelocityField`] — et se moque de savoir si
//! la vitesse vient d'une formule ou, plus tard, d'un calcul de potentiel. C'est le
//! rôle d'un trait : nommer ce dont on a besoin, pas ce dont on dispose.

use crate::geom::{Point, Vec2};

/// Un champ de vitesse stationnaire et incompressible.
///
/// `Sync` est exigé dès maintenant pour que le champ puisse être lu depuis plusieurs
/// threads à l'étape de parallélisation, sans avoir à modifier le contrat ensuite.
pub trait VelocityField: Sync {
    /// Vitesse au point donné.
    fn at(&self, p: Point) -> Vec2;

    /// Fonction de courant `ψ`, telle que `u = (∂ψ/∂y, −∂ψ/∂x)`.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cylindre() -> PotentialCylinder {
        PotentialCylinder {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
            speed: 2.0,
            circulation: 0.0,
        }
    }

    #[test]
    fn vitesse_uniforme_loin_de_l_obstacle() {
        let c = cylindre();
        let far = c.at(Point::new(1000.0, 0.0));
        assert!((far.x - c.speed).abs() < 1e-4);
        assert!(far.y.abs() < 1e-4);
    }

    #[test]
    fn glissement_sur_la_paroi_du_cylindre() {
        // sans circulation, la composante normale doit être nulle sur r = R
        let c = cylindre();
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
    fn la_fonction_de_courant_redonne_la_vitesse() {
        let mut c = cylindre();
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
    fn la_circulation_casse_la_symetrie() {
        let mut c = cylindre();
        c.circulation = 4.0;
        let haut = c.at(Point::new(0.0, 1.5));
        let bas = c.at(Point::new(0.0, -1.5));
        // circulation positive (sens direct) : l'écoulement est freiné au-dessus,
        // accéléré en dessous — la portance change de côté avec le signe de Γ
        assert!(bas.x > haut.x + 0.1, "haut = {haut:?}, bas = {bas:?}");
    }
}
