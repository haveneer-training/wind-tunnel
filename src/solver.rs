//! Solveur volumes finis explicite pour le transport d'un traceur.
//!
//! La mise à jour est écrite en *gather* : on boucle sur les cellules, et chaque cellule
//! lit ses faces pour accumuler son propre résidu. Personne n'écrit chez le voisin.
//!
//! Ce détail d'écriture est le sujet de toute une étape ultérieure. La formulation
//! duale, en *scatter* — boucler sur les faces et ajouter le flux aux deux cellules
//! adjacentes — est plus naturelle à écrire, plus économe en calcul… et devient une
//! course de données dès qu'on la parallélise. En C ou en Fortran avec OpenMP, elle se
//! parallélise silencieusement et donne un résultat faux de façon non reproductible.
//! En Rust, elle ne compile pas.

use std::collections::BTreeMap;

#[cfg(feature = "step9")]
#[cfg_attr(not(feature = "step10"), allow(unused_imports))] // collatéral du trou étape 9
use rayon::prelude::*;

use crate::error::SolverError;
use crate::field::Field;
#[cfg_attr(not(feature = "step10"), allow(unused_imports))] // collatéral du trou étape 9
use crate::flux::{FaceState, FluxScheme};
// `Vec2` et `gradient` ne servent qu'aux versions ≥ étape 7 de `residual` : plutôt que
// de taire l'avertissement, on conditionne l'import lui-même.
#[cfg(feature = "step7")]
use crate::geom::Vec2;
#[cfg(feature = "step7")]
use crate::gradient;
#[cfg_attr(not(feature = "step10"), allow(unused_imports))] // collatéral du trou étape 9
use crate::mesh::{BoundaryKind, CellId, Mesh, Side};
use crate::velocity::VelocityField;

/// Schéma d'intégration en temps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TimeScheme {
    /// Euler explicite : une évaluation du résidu par pas, ordre 1 en temps.
    #[default]
    Euler,
    /// Runge-Kutta d'ordre 2 (méthode de Heun) : deux évaluations du résidu par pas,
    /// ordre 2 en temps.
    Rk2,
}

/// Condition aux limites appliquée à un groupe de faces de bord.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bc {
    /// Valeur imposée du traceur (entrée).
    Fixed(f64),
    /// Gradient nul : le traceur sort librement.
    ZeroGradient,
    /// Flux nul : paroi imperméable.
    NoFlux,
}

/// Paramètres d'un calcul.
#[derive(Clone, Debug)]
pub struct Config {
    /// Diffusivité `D` du traceur.
    pub diffusivity: f64,
    /// Fraction du pas de temps maximal à utiliser, si `dt` n'est pas imposé.
    pub cfl: f64,
    /// Pas de temps imposé. `None` pour le déduire de la condition CFL.
    pub dt: Option<f64>,
    /// Nombre de pas de temps.
    pub steps: usize,
    /// Période de sortie, en pas de temps (`0` pour ne rien sortir).
    pub output_every: usize,
    /// Période de vérification de la finitude du champ (`0` pour ne pas vérifier).
    pub check_every: usize,
    /// Schéma d'intégration en temps.
    pub time_scheme: TimeScheme,
    /// Conditions aux limites, par nature de bord.
    pub bc: BTreeMap<BoundaryKind, Bc>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            diffusivity: 0.0,
            cfl: 0.4,
            dt: None,
            steps: 600,
            output_every: 10,
            check_every: 20,
            time_scheme: TimeScheme::Euler,
            // Les parois sont en gradient nul, pas en flux nul, et ce n'est pas un
            // oubli : une paroi en escalier n'est pas exactement une ligne de courant
            // de l'écoulement analytique, donc un petit débit résiduel la traverse.
            // Le supprimer fabriquerait une divergence artificielle et ferait perdre
            // au schéma sa propriété de borne ; on préfère laisser ce débit emporter
            // la valeur locale. L'étape 12, qui calcule l'écoulement sur le maillage
            // lui-même, supprime le résidu à la source.
            bc: BTreeMap::from([
                (BoundaryKind::Inlet, Bc::Fixed(0.0)),
                (BoundaryKind::Outlet, Bc::ZeroGradient),
                (BoundaryKind::Wall, Bc::ZeroGradient),
                (BoundaryKind::Obstacle, Bc::ZeroGradient),
            ]),
        }
    }
}

/// Un solveur prêt à tourner sur un maillage donné.
///
/// Le solveur emprunte son maillage (`'m`) : il ne le possède pas, ne le copie pas, et
/// le compilateur garantit que le maillage vivra plus longtemps que le solveur.
pub struct Solver<'m, F: FluxScheme> {
    mesh: &'m Mesh,
    config: Config,
    #[cfg_attr(not(feature = "step10"), allow(dead_code))] // collatéral du trou étape 9
    scheme: F,
    /// Débit volumique `∫ u·n dl` de chaque face, sortant de `face.left`.
    ///
    /// Calculé une fois pour toutes — l'écoulement est stationnaire — et par
    /// différence de fonction de courant, ce qui rend le bilan de débit d'une cellule
    /// exactement nul.
    face_flux: Vec<f64>,
    dt: f64,
    dt_max: f64,
}

impl<'m, F: FluxScheme> Solver<'m, F> {
    /// Prépare un calcul, en vérifiant d'emblée la stabilité du pas de temps.
    ///
    /// Une intégration explicite instable ne donne pas un résultat approximatif : elle
    /// donne du bruit, puis des `NaN`. Autant le dire avant les dix mille pas de temps
    /// plutôt qu'après.
    pub fn new(
        mesh: &'m Mesh,
        velocity: &dyn VelocityField,
        scheme: F,
        config: Config,
    ) -> Result<Self, SolverError> {
        let face_flux = compute_face_flux(mesh, velocity);
        // `None` : aucune cellule n'a de débit sortant ni de diffusion. Plutôt que de
        // laisser filer une valeur sentinelle — `f64::MAX`, dont `0.4 × f64::MAX` ferait
        // un pas de temps de 10³⁰⁸ secondes et une date physique qui déborde — on le dit
        // dans le type de retour, et l'appelant est obligé d'en tenir compte.
        let dt_max =
            max_stable_dt(mesh, &face_flux, config.diffusivity).ok_or(SolverError::NoTransport)?;
        let dt = config.dt.unwrap_or(config.cfl * dt_max);
        if dt > dt_max * (1.0 + 1e-12) {
            return Err(SolverError::Cfl { dt, dt_max });
        }
        Ok(Solver {
            mesh,
            config,
            scheme,
            face_flux,
            dt,
            dt_max,
        })
    }

    /// Le maillage sur lequel tourne le solveur.
    pub fn mesh(&self) -> &'m Mesh {
        self.mesh
    }

    /// Pas de temps effectivement utilisé.
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Pas de temps maximal admissible par la condition CFL.
    pub fn max_stable_dt(&self) -> f64 {
        self.dt_max
    }

    /// Bilan de débit d'une cellule : `Σ_f débit sortant`.
    ///
    /// Vaut zéro à la précision machine, par construction des débits.
    pub fn divergence(&self, id: CellId) -> f64 {
        self.mesh
            .cell_faces(id)
            .iter()
            .map(|&fid| {
                let face = self.mesh.face(fid);
                let outward = if face.left == id { 1.0 } else { -1.0 };
                self.face_flux[fid.index()] * outward
            })
            .sum()
    }

    /// Condition aux limites d'un bord ; imperméable par défaut.
    #[cfg_attr(not(feature = "step10"), allow(dead_code))] // collatéral du trou étape 9
    fn bc(&self, kind: BoundaryKind) -> Bc {
        self.config.bc.get(&kind).copied().unwrap_or(Bc::NoFlux)
    }

    /// Calcule `dc/dt` dans `out`.
    ///
    /// `c` est emprunté en lecture, `out` en écriture exclusive : le compilateur refuse
    /// qu'on lui passe deux fois le même champ. En C, le même appel avec le même
    /// pointeur des deux côtés compile, tourne, et donne un résultat faux ; `restrict`
    /// ne fait que promettre le contraire.
    ///
    /// Depuis l'étape 7, un tampon de gradients est alloué ici à chaque appel — `c`
    /// change à chaque pas de temps, donc le gradient aussi, contrairement à
    /// `face_flux` qui ne dépend que du maillage. `work`, lui, reste réutilisé d'un pas
    /// à l'autre par [`Solver::run`] : faire de même pour les gradients est une bonne
    /// extension (voir `docs/etapes/etape-07.md`).
    #[cfg(feature = "step9")]
    pub fn residual(&self, c: &Field, out: &mut Field) {
        let mut gradients = vec![Vec2::ZERO; c.len()];
        gradient::limited_gradients(self.mesh, c, &mut gradients);

        // TODO-STEP:9 Paralléliser cette boucle avec rayon. Chaque cellule ne lit que
        // `self`, `c` et `gradients` (partagés, en lecture seule) et n'écrit que sa
        // propre case de `out` : `par_iter()` sur les cellules, `zip`é avec `out` en
        // écriture (`par_iter_mut()`), `enumerate()` pour retrouver l'identifiant.
        // SOLUTION-BEGIN
        self.mesh
            .cells()
            .par_iter()
            .zip(out.as_mut_slice().par_iter_mut())
            .enumerate()
            .for_each(|(i, (cell, out_value))| {
                let id = CellId(i as u32);
                let ci = c[id];
                let mut sum = 0.0;

                for &fid in self.mesh.cell_faces(id) {
                    let face = self.mesh.face(fid);
                    // Le débit stocké est sortant de `face.left` : on le retourne si la
                    // cellule courante se trouve de l'autre côté.
                    let outward = if face.left == id { 1.0 } else { -1.0 };
                    let flux = self.face_flux[fid.index()] * outward;
                    let un = flux / face.length;
                    let to_face_left = face.midpoint - cell.centroid;

                    let (c_other, grad_right, to_face_right) = match face.right {
                        Side::Inner(other) => {
                            let neighbour = if face.left == id { other } else { face.left };
                            let n_centroid = self.mesh.cell(neighbour).centroid;
                            (
                                c[neighbour],
                                Some(gradients[neighbour.index()]),
                                face.midpoint - n_centroid,
                            )
                        }
                        Side::Boundary(kind) => match self.bc(kind) {
                            Bc::NoFlux => continue,
                            Bc::Fixed(value) => (value, None, Vec2::ZERO),
                            Bc::ZeroGradient => (ci, None, Vec2::ZERO),
                        },
                    };

                    let c_face = self.scheme.interface_value(&FaceState {
                        c_left: ci,
                        c_right: c_other,
                        un,
                        grad_left: gradients[id.index()],
                        to_face_left,
                        grad_right,
                        to_face_right,
                    });
                    // Le terme convectif utilise le débit tel quel : c'est lui qui se
                    // télescope exactement sur le contour de la cellule.
                    sum += flux * c_face
                        - self.config.diffusivity * (c_other - ci) / face.distance * face.length;
                }

                *out_value = -sum / cell.area;
            });
        // SOLUTION-END
    }

    /// Calcule `dc/dt` dans `out` — version séquentielle, entre les étapes 7 et 9.
    ///
    /// `c` est emprunté en lecture, `out` en écriture exclusive : le compilateur refuse
    /// qu'on lui passe deux fois le même champ. En C, le même appel avec le même
    /// pointeur des deux côtés compile, tourne, et donne un résultat faux ; `restrict`
    /// ne fait que promettre le contraire.
    #[cfg(all(feature = "step7", not(feature = "step9")))]
    pub fn residual(&self, c: &Field, out: &mut Field) {
        let mut gradients = vec![Vec2::ZERO; c.len()];
        gradient::limited_gradients(self.mesh, c, &mut gradients);

        for (i, cell) in self.mesh.cells().iter().enumerate() {
            let id = CellId(i as u32);
            let ci = c[id];
            let mut sum = 0.0;

            for &fid in self.mesh.cell_faces(id) {
                let face = self.mesh.face(fid);
                // Le débit stocké est sortant de `face.left` : on le retourne si la
                // cellule courante se trouve de l'autre côté.
                let outward = if face.left == id { 1.0 } else { -1.0 };
                let flux = self.face_flux[fid.index()] * outward;
                let un = flux / face.length;
                let to_face_left = face.midpoint - cell.centroid;

                let (c_other, grad_right, to_face_right) = match face.right {
                    Side::Inner(other) => {
                        let neighbour = if face.left == id { other } else { face.left };
                        let n_centroid = self.mesh.cell(neighbour).centroid;
                        (
                            c[neighbour],
                            Some(gradients[neighbour.index()]),
                            face.midpoint - n_centroid,
                        )
                    }
                    Side::Boundary(kind) => match self.bc(kind) {
                        Bc::NoFlux => continue,
                        Bc::Fixed(value) => (value, None, Vec2::ZERO),
                        Bc::ZeroGradient => (ci, None, Vec2::ZERO),
                    },
                };

                let c_face = self.scheme.interface_value(&FaceState {
                    c_left: ci,
                    c_right: c_other,
                    un,
                    grad_left: gradients[id.index()],
                    to_face_left,
                    grad_right,
                    to_face_right,
                });
                // Le terme convectif utilise le débit tel quel : c'est lui qui se
                // télescope exactement sur le contour de la cellule.
                sum += flux * c_face
                    - self.config.diffusivity * (c_other - ci) / face.distance * face.length;
            }

            out[id] = -sum / cell.area;
        }
    }

    /// Calcule `dc/dt` dans `out`, sans rien allouer — version d'avant l'étape 7 (pas
    /// de reconstruction de gradient, `FaceState` prend ses valeurs par défaut).
    ///
    /// `c` est emprunté en lecture, `out` en écriture exclusive : le compilateur refuse
    /// qu'on lui passe deux fois le même champ. En C, le même appel avec le même
    /// pointeur des deux côtés compile, tourne, et donne un résultat faux ; `restrict`
    /// ne fait que promettre le contraire.
    #[cfg(not(feature = "step7"))]
    pub fn residual(&self, c: &Field, out: &mut Field) {
        for (i, cell) in self.mesh.cells().iter().enumerate() {
            let id = CellId(i as u32);
            let ci = c[id];
            let mut sum = 0.0;

            for &fid in self.mesh.cell_faces(id) {
                let face = self.mesh.face(fid);
                // Le débit stocké est sortant de `face.left` : on le retourne si la
                // cellule courante se trouve de l'autre côté.
                let outward = if face.left == id { 1.0 } else { -1.0 };
                let flux = self.face_flux[fid.index()] * outward;
                let un = flux / face.length;

                let c_other = match face.right {
                    Side::Inner(other) => {
                        let neighbour = if face.left == id { other } else { face.left };
                        c[neighbour]
                    }
                    Side::Boundary(kind) => match self.bc(kind) {
                        Bc::NoFlux => continue,
                        Bc::Fixed(value) => value,
                        Bc::ZeroGradient => ci,
                    },
                };

                let c_face = self.scheme.interface_value(&FaceState {
                    c_left: ci,
                    c_right: c_other,
                    un,
                    ..Default::default()
                });
                // Le terme convectif utilise le débit tel quel : c'est lui qui se
                // télescope exactement sur le contour de la cellule.
                sum += flux * c_face
                    - self.config.diffusivity * (c_other - ci) / face.distance * face.length;
            }

            out[id] = -sum / cell.area;
        }
    }

    /// Avance d'un pas de temps, selon le schéma d'intégration choisi.
    ///
    /// `work` est un tampon fourni par l'appelant et réutilisé d'un pas à l'autre :
    /// la boucle en temps n'y alloue rien. `step_rk2`, lui, alloue en interne son
    /// prédicteur et son second résidu — comme `residual` alloue son tampon de
    /// gradients (étape 7) : voir « Pour aller plus loin » de l'étape 8.
    pub fn step(&self, c: &mut Field, work: &mut Field) {
        match self.config.time_scheme {
            TimeScheme::Euler => self.step_euler(c, work),
            TimeScheme::Rk2 => self.step_rk2(c, work),
        }
    }

    /// Euler explicite : une évaluation du résidu, ordre 1 en temps.
    #[cfg_attr(not(feature = "step5"), allow(unused_variables))] // trou étape 5
    fn step_euler(&self, c: &mut Field, work: &mut Field) {
        // TODO-STEP:5 Calculer le résidu dans `work`, puis avancer `c` de `dt · résidu`
        // SOLUTION-BEGIN
        self.residual(c, work);
        for (value, rate) in c.as_mut_slice().iter_mut().zip(work.as_slice()) {
            *value += self.dt * rate;
        }
        // SOLUTION-END
    }

    /// Runge-Kutta d'ordre 2 (méthode de Heun) : deux évaluations du résidu, ordre 2
    /// en temps. `work` reçoit `k1`.
    #[cfg_attr(not(feature = "step8"), allow(unused_variables))] // trou étape 8
    fn step_rk2(&self, c: &mut Field, work: &mut Field) {
        // TODO-STEP:8 k1 = résidu(c) dans `work` ; prédicteur = c + dt·k1 ; k2 =
        // résidu(prédicteur) ; avancer c de dt/2 · (k1 + k2)
        // SOLUTION-BEGIN
        self.residual(c, work);

        let mut predictor = c.clone();
        for (value, k1) in predictor.as_mut_slice().iter_mut().zip(work.as_slice()) {
            *value += self.dt * k1;
        }

        let mut k2 = Field::zeros(c.len());
        self.residual(&predictor, &mut k2);

        for ((value, k1), k2) in c
            .as_mut_slice()
            .iter_mut()
            .zip(work.as_slice())
            .zip(k2.as_slice())
        {
            *value += 0.5 * self.dt * (k1 + k2);
        }
        // SOLUTION-END
    }

    /// Déroule le calcul, en appelant `observer` à la période demandée.
    ///
    /// L'observateur reçoit le numéro de pas, la date physique et le champ courant.
    pub fn run(
        &self,
        c: &mut Field,
        mut observer: impl FnMut(usize, f64, &Field) -> Result<(), SolverError>,
    ) -> Result<(), SolverError> {
        let mut work = Field::zeros(c.len());
        observer(0, 0.0, c)?;

        for step in 1..=self.config.steps {
            self.step(c, &mut work);

            if self.config.check_every > 0 && step % self.config.check_every == 0 {
                if let Some(cell) = c.first_non_finite() {
                    return Err(SolverError::NotFinite { step, cell });
                }
            }
            if self.config.output_every > 0 && step % self.config.output_every == 0 {
                observer(step, step as f64 * self.dt, c)?;
            }
        }
        Ok(())
    }
}

/// Débit volumique de chaque face, par différence de fonction de courant (voir le champ
/// `face_flux` de [`Solver`]).
#[cfg(feature = "step9")]
fn compute_face_flux(mesh: &Mesh, velocity: &dyn VelocityField) -> Vec<f64> {
    let vertices = mesh.vertices();
    // TODO-STEP:9 Paralléliser ce calcul avec rayon. C'est un `map` sur les faces,
    // chacune indépendante des autres : `.iter()` → `.par_iter()` suffit.
    // SOLUTION-BEGIN
    mesh.faces()
        .par_iter()
        .map(|f| velocity.stream(vertices[f.b.index()]) - velocity.stream(vertices[f.a.index()]))
        .collect()
    // SOLUTION-END
}

/// Débit volumique de chaque face, par différence de fonction de courant (voir le champ
/// `face_flux` de [`Solver`]).
#[cfg(not(feature = "step9"))]
fn compute_face_flux(mesh: &Mesh, velocity: &dyn VelocityField) -> Vec<f64> {
    let vertices = mesh.vertices();
    mesh.faces()
        .iter()
        .map(|f| velocity.stream(vertices[f.b.index()]) - velocity.stream(vertices[f.a.index()]))
        .collect()
}

/// Pas de temps maximal admissible : `min_i |Ωi| / Σ_f (|débit| + 2D L_f/d_f)`.
///
/// `None` si *aucune* cellule n'a de débit sortant ni de diffusion : il n'y a alors rien
/// à transporter, et le minimum porterait sur un ensemble vide.
#[cfg_attr(not(feature = "step5"), allow(unused_variables))] // trou étape 5
fn max_stable_dt(mesh: &Mesh, face_flux: &[f64], diffusivity: f64) -> Option<f64> {
    // TODO-STEP:5 Pour chaque cellule, sommer sur ses faces `|débit| + 2·D·L/d`, puis
    // retenir le plus petit rapport `aire / somme` du maillage. Renvoyer `None` si
    // aucune cellule n'a de somme strictement positive.
    // SOLUTION-BEGIN
    // `filter_map` écarte les cellules sans transport, `reduce` prend le minimum de ce
    // qui reste — et renvoie `None` s'il ne reste rien, sans qu'on ait à le traiter à
    // part. C'est là tout l'intérêt de `reduce` face à `fold` : pas de valeur initiale
    // à inventer, donc pas de sentinelle à faire passer pour un résultat.
    mesh.cells()
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| {
            let id = CellId(i as u32);
            let outflow: f64 = mesh
                .cell_faces(id)
                .iter()
                .map(|&fid| {
                    let face = mesh.face(fid);
                    face_flux[fid.index()].abs() + 2.0 * diffusivity * face.length / face.distance
                })
                .sum();
            (outflow > 0.0).then(|| cell.area / outflow)
        })
        .reduce(f64::min)
    // SOLUTION-END
}

#[cfg(all(test, feature = "step5"))]
mod tests {
    use super::*;
    use crate::flux::Upwind;
    use crate::geom::Vec2;
    use crate::mask::Mask;
    use crate::velocity::Uniform;

    fn test_mesh() -> Mesh {
        Mesh::from_mask(&Mask::parse("....\n....\n....\n").unwrap(), 1.0).unwrap()
    }

    #[test]
    fn unstable_time_step_is_rejected() {
        let mesh = test_mesh();
        let flow = Uniform {
            value: Vec2::new(1.0, 0.0),
        };
        let config = Config {
            dt: Some(1e3),
            ..Config::default()
        };
        match Solver::new(&mesh, &flow, Upwind, config) {
            Err(SolverError::Cfl { dt, dt_max }) => assert!(dt > dt_max),
            Err(other) => panic!("erreur inattendue : {other}"),
            Ok(_) => panic!("un pas de temps de 1000 s aurait dû être refusé"),
        }
    }

    #[test]
    fn a_motionless_case_is_rejected() {
        // vitesse nulle et diffusivité nulle : plus rien ne transporte quoi que ce
        // soit, et la CFL n'impose aucune borne. Refuser vaut mieux que renvoyer un
        // pas de temps de 10³⁰⁸ secondes.
        let mesh = test_mesh();
        let flow = Uniform { value: Vec2::ZERO };
        let config = Config {
            diffusivity: 0.0,
            ..Config::default()
        };
        match Solver::new(&mesh, &flow, Upwind, config) {
            Err(SolverError::NoTransport) => {}
            Err(other) => panic!("erreur inattendue : {other}"),
            Ok(solver) => panic!("pas de temps {} accepté sans transport", solver.dt()),
        }
    }

    #[test]
    fn a_uniform_field_stays_uniform() {
        // avec la même valeur imposée en entrée qu'à l'intérieur, rien ne doit bouger
        let mesh = test_mesh();
        let flow = Uniform {
            value: Vec2::new(1.0, 0.0),
        };
        let config = Config {
            steps: 50,
            output_every: 0,
            bc: BTreeMap::from([
                (BoundaryKind::Inlet, Bc::Fixed(1.0)),
                (BoundaryKind::Outlet, Bc::ZeroGradient),
                (BoundaryKind::Wall, Bc::ZeroGradient),
            ]),
            ..Config::default()
        };
        let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();
        let mut c = Field::filled(mesh.n_cells(), 1.0);
        solver.run(&mut c, |_, _, _| Ok(())).unwrap();
        let (lo, hi) = c.min_max();
        assert!((lo - 1.0).abs() < 1e-12 && (hi - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_non_finite_value_stops_the_run() {
        let mesh = test_mesh();
        let flow = Uniform {
            value: Vec2::new(1.0, 0.0),
        };
        let config = Config {
            steps: 10,
            output_every: 0,
            check_every: 1,
            ..Config::default()
        };
        let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();
        let mut c = Field::zeros(mesh.n_cells());
        c[CellId(3)] = f64::INFINITY;
        match solver.run(&mut c, |_, _, _| Ok(())) {
            Err(SolverError::NotFinite { step, .. }) => assert_eq!(step, 1),
            other => panic!("la divergence n'a pas été détectée : {other:?}"),
        }
    }

    #[test]
    fn face_fluxes_are_divergence_free() {
        // y compris autour d'un obstacle, où les cellules sont chanfreinées
        let mesh = Mesh::from_mask(
            &Mask::parse(".......\n...##..\n...##..\n.......\n").unwrap(),
            0.5,
        )
        .unwrap();
        let flow = crate::velocity::PotentialCylinder {
            center: crate::geom::Point::new(2.0, 1.0),
            radius: 0.5,
            speed: 1.0,
            circulation: 0.7,
        };
        let solver = Solver::new(&mesh, &flow, Upwind, Config::default()).unwrap();
        for i in 0..mesh.n_cells() {
            let id = CellId(i as u32);
            let div = solver.divergence(id);
            assert!(div.abs() < 1e-12, "cellule {i} : divergence {div:e}");
        }
    }

    #[test]
    fn pure_diffusion_conserves_mass() {
        // domaine fermé, aucune vitesse : la masse totale est un invariant exact
        let mesh = test_mesh();
        let flow = Uniform { value: Vec2::ZERO };
        let config = Config {
            diffusivity: 0.1,
            steps: 200,
            output_every: 0,
            bc: BTreeMap::new(), // tout en flux nul, par défaut
            ..Config::default()
        };
        let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();
        let mut c = Field::from_fn(&mesh, |id| if id.index() % 3 == 0 { 1.0 } else { 0.0 });
        let before = c.total_mass(&mesh);
        solver.run(&mut c, |_, _, _| Ok(())).unwrap();
        let after = c.total_mass(&mesh);
        assert!(
            (after - before).abs() / before < 1e-13,
            "masse {before} → {after}"
        );
    }
}
