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

use crate::error::SolverError;
use crate::field::Field;
use crate::flux::{FaceState, FluxScheme};
use crate::mesh::{BoundaryKind, CellId, Mesh, Side};
use crate::velocity::VelocityField;

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
            // Les parois sont en gradient nul, pas en flux nul, et ce n'est pas un
            // oubli : une paroi en escalier n'est pas exactement une ligne de courant
            // de l'écoulement analytique, donc un petit débit résiduel la traverse.
            // Le supprimer fabriquerait une divergence artificielle et ferait perdre
            // au schéma sa propriété de borne ; on préfère laisser ce débit emporter
            // la valeur locale. L'étape 11, qui calcule l'écoulement sur le maillage
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
        let vertices = mesh.vertices();
        let face_flux: Vec<f64> = mesh
            .faces()
            .iter()
            .map(|f| {
                velocity.stream(vertices[f.b.index()]) - velocity.stream(vertices[f.a.index()])
            })
            .collect();
        let dt_max = max_stable_dt(mesh, &face_flux, config.diffusivity);
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
    fn bc(&self, kind: BoundaryKind) -> Bc {
        self.config.bc.get(&kind).copied().unwrap_or(Bc::NoFlux)
    }

    /// Calcule `dc/dt` dans `out`, sans rien allouer.
    ///
    /// `c` est emprunté en lecture, `out` en écriture exclusive : le compilateur refuse
    /// qu'on lui passe deux fois le même champ. En C, le même appel avec le même
    /// pointeur des deux côtés compile, tourne, et donne un résultat faux ; `restrict`
    /// ne fait que promettre le contraire.
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
                });
                // Le terme convectif utilise le débit tel quel : c'est lui qui se
                // télescope exactement sur le contour de la cellule.
                sum += flux * c_face
                    - self.config.diffusivity * (c_other - ci) / face.distance * face.length;
            }

            out[id] = -sum / cell.area;
        }
    }

    /// Avance d'un pas de temps par la méthode d'Euler explicite.
    ///
    /// `work` est un tampon fourni par l'appelant et réutilisé d'un pas à l'autre :
    /// la boucle en temps n'alloue rien.
    pub fn step(&self, c: &mut Field, work: &mut Field) {
        // TODO-STEP:5 Calculer le résidu dans `work`, puis avancer `c` de `dt · résidu`
        // SOLUTION-BEGIN
        self.residual(c, work);
        for (value, rate) in c.as_mut_slice().iter_mut().zip(work.as_slice()) {
            *value += self.dt * rate;
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

/// Pas de temps maximal admissible : `min_i |Ωi| / Σ_f (|débit| + 2D L_f/d_f)`.
fn max_stable_dt(mesh: &Mesh, face_flux: &[f64], diffusivity: f64) -> f64 {
    // TODO-STEP:5 Pour chaque cellule, sommer sur ses faces `|débit| + 2·D·L/d`, puis
    // retenir le plus petit rapport `aire / somme` du maillage
    // SOLUTION-BEGIN
    let mut dt = f64::MAX;
    for (i, cell) in mesh.cells().iter().enumerate() {
        let id = CellId(i as u32);
        let outflow: f64 = mesh
            .cell_faces(id)
            .iter()
            .map(|&fid| {
                let face = mesh.face(fid);
                face_flux[fid.index()].abs() + 2.0 * diffusivity * face.length / face.distance
            })
            .sum();
        if outflow > 0.0 {
            dt = dt.min(cell.area / outflow);
        }
    }
    dt
    // SOLUTION-END
}

#[cfg(test)]
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
