//! Géométrie 2D : points, vecteurs, aires et centroïdes de polygones.
//!
//! `Point` et `Vec2` sont deux types distincts alors qu'ils portent les mêmes champs.
//! Ce n'est pas de la coquetterie : soustraire deux points donne un déplacement,
//! additionner deux points n'a aucun sens. Le compilateur le sait désormais aussi.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// Un point du plan, en coordonnées physiques.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Abscisse.
    pub x: f64,
    /// Ordonnée.
    pub y: f64,
}

/// Un déplacement dans le plan.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec2 {
    /// Composante selon x.
    pub x: f64,
    /// Composante selon y.
    pub y: f64,
}

impl Point {
    /// Construit un point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Milieu de deux points.
    pub fn midpoint(self, other: Point) -> Point {
        Point::new(0.5 * (self.x + other.x), 0.5 * (self.y + other.y))
    }

    /// Distance euclidienne entre deux points.
    pub fn distance(self, other: Point) -> f64 {
        (other - self).norm()
    }
}

impl Vec2 {
    /// Le vecteur nul.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// Construit un vecteur.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Produit scalaire.
    ///
    /// ```
    /// use wind_tunnel::geom::Vec2;
    /// assert_eq!(Vec2::new(3.0, 4.0).dot(Vec2::new(1.0, 0.0)), 3.0);
    /// ```
    pub fn dot(self, other: Vec2) -> f64 {
        // TODO-STEP:0 Produit scalaire de deux vecteurs du plan
        // SOLUTION-BEGIN
        self.x * other.x + self.y * other.y
        // SOLUTION-END
    }

    /// Norme euclidienne.
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Rotation de −90°.
    ///
    /// Pour une arête `a → b` d'un polygone parcouru dans le sens direct
    /// (trigonométrique), ce vecteur pointe vers l'extérieur du polygone.
    ///
    /// ```
    /// use wind_tunnel::geom::Vec2;
    /// // arête du bas d'un carré, parcourue vers la droite : la normale sortante pointe vers le bas
    /// assert_eq!(Vec2::new(1.0, 0.0).perp_cw(), Vec2::new(0.0, -1.0));
    /// ```
    pub fn perp_cw(self) -> Vec2 {
        // TODO-STEP:0 Tourner le vecteur de −90°
        // SOLUTION-BEGIN
        Vec2::new(self.y, -self.x)
        // SOLUTION-END
    }

    /// Vecteur unitaire de même direction ; renvoie [`Vec2::ZERO`] si le vecteur est nul.
    pub fn normalized(self) -> Vec2 {
        let n = self.norm();
        if n == 0.0 {
            Vec2::ZERO
        } else {
            self / n
        }
    }
}

impl Sub for Point {
    type Output = Vec2;
    fn sub(self, other: Point) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl Add<Vec2> for Point {
    type Output = Point;
    fn add(self, v: Vec2) -> Point {
        Point::new(self.x + v.x, self.y + v.y)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f64) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Itère sur les arêtes `(sommet, sommet suivant)` d'un polygone fermé.
#[cfg_attr(not(feature = "step1"), allow(dead_code))] // collatéral du trou étape 0
fn edges(points: &[Point]) -> impl Iterator<Item = (&Point, &Point)> {
    points.iter().zip(points.iter().cycle().skip(1))
}

/// Aire signée d'un polygone simple (formule du lacet).
///
/// Positive si les sommets sont donnés dans le sens direct.
///
/// ```
/// use wind_tunnel::geom::{polygon_area, Point};
/// let square = [
///     Point::new(0.0, 0.0),
///     Point::new(2.0, 0.0),
///     Point::new(2.0, 2.0),
///     Point::new(0.0, 2.0),
/// ];
/// assert!((polygon_area(&square) - 4.0).abs() < 1e-15);
/// ```
pub fn polygon_area(points: &[Point]) -> f64 {
    // TODO-STEP:0 Aire signée par la formule du lacet : ½ Σ (xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)
    // SOLUTION-BEGIN
    0.5 * edges(points)
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f64>()
    // SOLUTION-END
}

/// Centroïde (barycentre géométrique) d'un polygone simple.
///
/// Pour un polygone d'aire négligeable, renvoie la moyenne des sommets.
///
/// ```
/// use wind_tunnel::geom::{polygon_centroid, Point};
/// let triangle = [Point::new(0.0, 0.0), Point::new(3.0, 0.0), Point::new(0.0, 3.0)];
/// let g = polygon_centroid(&triangle);
/// assert!((g.x - 1.0).abs() < 1e-12 && (g.y - 1.0).abs() < 1e-12);
/// ```
pub fn polygon_centroid(points: &[Point]) -> Point {
    // TODO-STEP:0 Centroïde : (1/6A) Σ (pᵢ + pᵢ₊₁)·(xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ), avec repli sur la
    // moyenne des sommets quand l'aire est négligeable
    // SOLUTION-BEGIN
    let area = polygon_area(points);
    if area.abs() < 1e-16 {
        let n = points.len() as f64;
        let sum = points
            .iter()
            .fold(Vec2::ZERO, |acc, p| acc + Vec2::new(p.x, p.y));
        return Point::new(sum.x / n, sum.y / n);
    }
    let sum = edges(points).fold(Vec2::ZERO, |acc, (a, b)| {
        let cross = a.x * b.y - b.x * a.y;
        acc + Vec2::new((a.x + b.x) * cross, (a.y + b.y) * cross)
    });
    Point::new(sum.x / (6.0 * area), sum.y / (6.0 * area))
    // SOLUTION-END
}

#[cfg(all(test, feature = "step0"))]
mod tests {
    use super::*;

    #[test]
    fn area_of_unit_square() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert!((polygon_area(&square) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn area_is_negative_when_clockwise() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(1.0, 1.0),
            Point::new(1.0, 0.0),
        ];
        assert!(polygon_area(&square) < 0.0);
    }

    #[test]
    fn centroid_of_square() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let g = polygon_centroid(&square);
        assert!((g.x - 1.0).abs() < 1e-12);
        assert!((g.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn outward_normal_of_ccw_square() {
        // arête droite du carré, parcourue vers le haut : normale sortante vers +x
        let n = (Point::new(1.0, 1.0) - Point::new(1.0, 0.0))
            .perp_cw()
            .normalized();
        assert!((n.x - 1.0).abs() < 1e-15 && n.y.abs() < 1e-15);
    }
}
