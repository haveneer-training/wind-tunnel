//! Maillage non structuré : cellules, faces, connectivité.
//!
//! Le maillage est *engendré* depuis une grille masquée, mais il est *stocké* et
//! *parcouru* comme un maillage non structuré : une cellule ne connaît pas ses voisins
//! par arithmétique d'indices, elle les connaît par ses faces. Tout ce qui est écrit
//! au-dessus fonctionnerait donc à l'identique sur un maillage lu depuis un fichier.
//!
//! Deux décisions de conception méritent qu'on s'y arrête.
//!
//! **Des indices, pas des pointeurs.** En C, une connectivité stockée sous forme de
//! `Cell*` devient un champ de mines dès que le tableau de cellules est agrandi par
//! `realloc` : les pointeurs déjà distribués pointent alors dans le vide, et le
//! programme continue comme si de rien n'était. Les indices, eux, survivent au
//! déplacement du tableau.
//!
//! **Des indices typés.** `CellId`, `FaceId` et `VertexId` sont trois types distincts.
//! En C ils seraient tous des `int`, et passer un numéro de face là où l'on attend une
//! cellule compilerait sans un mot. Ici, non.

#[cfg_attr(not(feature = "step2"), allow(unused_imports))] // trou étape 2
use std::collections::{BTreeMap, HashMap};

use crate::error::MeshError;
use crate::geom::{polygon_area, polygon_centroid, Point, Vec2};
use crate::mask::Mask;

/// Identifiant d'un sommet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexId(pub u32);

/// Identifiant d'une cellule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellId(pub u32);

/// Identifiant d'une face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceId(pub u32);

impl VertexId {
    /// Indice utilisable pour accéder à un tableau.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl CellId {
    /// Indice utilisable pour accéder à un tableau.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl FaceId {
    /// Indice utilisable pour accéder à un tableau.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Forme d'une cellule et sommets qui la composent, dans le sens direct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// Triangle : produit par le chanfreinage d'un coin du masque.
    Tri([VertexId; 3]),
    /// Quadrangle : le cas courant.
    Quad([VertexId; 4]),
}

impl CellKind {
    /// Sommets de la cellule, dans le sens direct.
    pub fn vertices(&self) -> &[VertexId] {
        match self {
            CellKind::Tri(v) => v,
            CellKind::Quad(v) => v,
        }
    }

    /// Code de type de cellule VTK (`5` = triangle, `9` = quadrangle).
    pub fn vtk_code(&self) -> u8 {
        match self {
            CellKind::Tri(_) => 5,
            CellKind::Quad(_) => 9,
        }
    }
}

/// Nature d'un bord du domaine.
///
/// L'ordre dérivé sert de clé de `BTreeMap` : les groupes de faces sont ainsi
/// toujours listés dans le même ordre, ce qui rend les sorties reproductibles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryKind {
    /// Entrée de la veine (bord gauche du domaine).
    Inlet,
    /// Sortie de la veine (bord droit).
    Outlet,
    /// Paroi de la veine (haut et bas).
    Wall,
    /// Paroi de l'obstacle.
    Obstacle,
}

/// Ce qui se trouve de l'autre côté d'une face.
///
/// Le choix d'une énumération plutôt que d'un `Option<CellId>` doublé d'un drapeau
/// rend l'état incohérent « ni voisin ni condition aux limites » inexprimable, et
/// force le `match` à traiter le bord.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Une cellule voisine.
    Inner(CellId),
    /// Le bord du domaine, et sa nature.
    Boundary(BoundaryKind),
}

/// Une cellule et sa géométrie précalculée.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// Forme et sommets.
    pub kind: CellKind,
    /// Centre de gravité.
    pub centroid: Point,
    /// Aire (strictement positive).
    pub area: f64,
}

/// Une face, vue depuis sa cellule de gauche.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    /// Premier sommet.
    pub a: VertexId,
    /// Second sommet.
    pub b: VertexId,
    /// Cellule à laquelle la normale est sortante.
    pub left: CellId,
    /// Ce qu'il y a de l'autre côté.
    pub right: Side,
    /// Normale unitaire, sortante de [`Face::left`].
    pub normal: Vec2,
    /// Longueur de la face.
    pub length: f64,
    /// Milieu de la face.
    pub midpoint: Point,
    /// Distance utilisée pour les gradients normaux : entre centroïdes pour une face
    /// interne, du centroïde au milieu de la face pour une face de bord.
    pub distance: f64,
}

impl Face {
    /// Cellule voisine, s'il y en a une.
    pub fn neighbor(&self) -> Option<CellId> {
        match self.right {
            Side::Inner(c) => Some(c),
            Side::Boundary(_) => None,
        }
    }

    /// La face est-elle sur le bord du domaine ?
    pub fn is_boundary(&self) -> bool {
        self.neighbor().is_none()
    }
}

/// Un maillage non structuré 2D.
#[derive(Clone, Debug)]
pub struct Mesh {
    vertices: Vec<Point>,
    cells: Vec<Cell>,
    faces: Vec<Face>,
    // Connectivité cellule → faces, au format CSR : les faces de la cellule `i` sont
    // `cell_face_indices[cell_face_offsets[i] .. cell_face_offsets[i + 1]]`.
    cell_face_offsets: Vec<u32>,
    cell_face_indices: Vec<FaceId>,
    groups: BTreeMap<BoundaryKind, Vec<FaceId>>,
}

impl Mesh {
    /// Engendre le maillage d'un masque, avec des cellules de côté `h`.
    ///
    /// Une cellule fluide dont exactement deux voisins orthogonaux perpendiculaires
    /// sont solides voit son coin coupé : elle devient un triangle. Le maillage est
    /// donc réellement mixte, et l'escalier du masque est adouci.
    pub fn from_mask(mask: &Mask, h: f64) -> Result<Mesh, MeshError> {
        let (rows, cols) = (mask.rows(), mask.cols());
        let mut vertices: Vec<Point> = Vec::new();
        let mut vertex_table = vec![u32::MAX; (rows + 1) * (cols + 1)];
        let mut cells: Vec<Cell> = Vec::new();

        for row in 0..rows {
            for col in 0..cols {
                if !mask.is_fluid(row, col) {
                    continue;
                }
                let (r, c) = (row as isize, col as isize);
                // Voisins dans le sens direct : bas, droite, haut, gauche.
                let (down, right) = (mask.is_obstacle(r + 1, c), mask.is_obstacle(r, c + 1));
                let (up, left) = (mask.is_obstacle(r - 1, c), mask.is_obstacle(r, c - 1));

                let corners = cell_corners(row, col, down, right, up, left);

                let ids: Vec<VertexId> = corners
                    .iter()
                    .map(|&(vr, vc)| {
                        vertex_id(vr, vc, rows, cols, h, &mut vertices, &mut vertex_table)
                    })
                    .collect();
                let points: Vec<Point> = ids.iter().map(|&v| vertices[v.index()]).collect();

                let area = polygon_area(&points);
                let id = CellId(cells.len() as u32);
                if area <= 0.0 {
                    return Err(MeshError::DegenerateCell { cell: id, area });
                }
                let kind = match ids.len() {
                    3 => CellKind::Tri([ids[0], ids[1], ids[2]]),
                    _ => CellKind::Quad([ids[0], ids[1], ids[2], ids[3]]),
                };
                cells.push(Cell {
                    kind,
                    centroid: polygon_centroid(&points),
                    area,
                });
            }
        }

        if cells.is_empty() {
            return Err(MeshError::EmptyDomain);
        }

        let faces = build_faces(&cells, &vertices)?;
        let mut mesh = Mesh {
            vertices,
            cells,
            faces,
            cell_face_offsets: Vec::new(),
            cell_face_indices: Vec::new(),
            groups: BTreeMap::new(),
        };
        mesh.classify_boundaries();
        mesh.finish_geometry();
        mesh.build_cell_faces();
        Ok(mesh)
    }

    /// Nombre de sommets.
    pub fn n_vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Nombre de cellules.
    pub fn n_cells(&self) -> usize {
        self.cells.len()
    }

    /// Nombre de faces.
    pub fn n_faces(&self) -> usize {
        self.faces.len()
    }

    /// Tous les sommets.
    pub fn vertices(&self) -> &[Point] {
        &self.vertices
    }

    /// Toutes les cellules.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Toutes les faces.
    pub fn faces(&self) -> &[Face] {
        &self.faces
    }

    /// Une cellule.
    pub fn cell(&self, id: CellId) -> &Cell {
        &self.cells[id.index()]
    }

    /// Une face.
    pub fn face(&self, id: FaceId) -> &Face {
        &self.faces[id.index()]
    }

    /// Les faces d'une cellule.
    pub fn cell_faces(&self, id: CellId) -> &[FaceId] {
        let start = self.cell_face_offsets[id.index()] as usize;
        let end = self.cell_face_offsets[id.index() + 1] as usize;
        &self.cell_face_indices[start..end]
    }

    /// Les faces de bord, groupées par nature.
    pub fn groups(&self) -> &BTreeMap<BoundaryKind, Vec<FaceId>> {
        &self.groups
    }

    /// Aire totale du domaine fluide.
    pub fn total_area(&self) -> f64 {
        self.cells.iter().map(|c| c.area).sum()
    }

    /// Nombre de triangles et de quadrangles.
    pub fn shape_counts(&self) -> (usize, usize) {
        let tri = self
            .cells
            .iter()
            .filter(|c| matches!(c.kind, CellKind::Tri(_)))
            .count();
        (tri, self.cells.len() - tri)
    }

    /// Translate tout le maillage de `delta`.
    ///
    /// Sert au découpage en bandes de l'étape 11 : [`Mesh::from_mask`] place toujours la
    /// première colonne du masque en `x = 0`, or la bande d'un rang commence ailleurs
    /// dans le domaine. On la remet à sa place, et l'écoulement analytique — évalué en
    /// coordonnées globales — redevient le bon.
    ///
    /// Longueurs, normales et distances sont invariantes par translation : seuls les
    /// points bougent.
    pub fn translate(&mut self, delta: Vec2) {
        for vertex in &mut self.vertices {
            *vertex = *vertex + delta;
        }
        for cell in &mut self.cells {
            cell.centroid = cell.centroid + delta;
        }
        for face in &mut self.faces {
            face.midpoint = face.midpoint + delta;
        }
    }

    /// Boîte englobante `(xmin, ymin, xmax, ymax)` du maillage.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        self.vertices.iter().fold(
            (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
            |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        )
    }

    /// Attribue une nature à chaque face de bord, d'après sa position.
    fn classify_boundaries(&mut self) {
        let (xmin, ymin, xmax, ymax) = self.bounds();
        let eps = 1e-9 * (xmax - xmin).max(ymax - ymin);
        let mut groups: BTreeMap<BoundaryKind, Vec<FaceId>> = BTreeMap::new();

        for (i, face) in self.faces.iter_mut().enumerate() {
            if !face.is_boundary() {
                continue;
            }
            let m = face.midpoint;
            let kind = if (m.x - xmin).abs() < eps {
                BoundaryKind::Inlet
            } else if (xmax - m.x).abs() < eps {
                BoundaryKind::Outlet
            } else if (m.y - ymin).abs() < eps || (ymax - m.y).abs() < eps {
                BoundaryKind::Wall
            } else {
                BoundaryKind::Obstacle
            };
            face.right = Side::Boundary(kind);
            groups.entry(kind).or_default().push(FaceId(i as u32));
        }
        self.groups = groups;
    }

    /// Vérifie qu'aucun sommet n'est partagé entre l'obstacle et le bord du domaine.
    ///
    /// Un tel sommet porterait deux natures à la fois : l'écoulement calculé
    /// (étape 12) impose à chaque sommet de bord une seule valeur de `ψ`, choisie
    /// selon la nature de ses faces (`ψ₀(y)` pour `Inlet`/`Wall`, une constante pour
    /// `Obstacle`). L'une des deux valeurs écraserait l'autre — plus une ligne de
    /// courant, un débit parasite s'installe et grossit sans borne au fil du temps.
    ///
    /// Volontairement **pas** appelée par [`Mesh::from_mask`] : une bande de l'étape 11
    /// ([`crate::decomposition::Layout::build_mesh`]) a pour bord légitime la coupure
    /// de la bande, qui peut tomber sur l'obstacle sans que cela pose problème (elle
    /// n'alimente jamais l'écoulement calculé). C'est au masque dessiné à la main —
    /// donc à l'appelant qui construit le maillage global — de demander cette
    /// vérification.
    pub fn check_obstacle_clear_of_boundary(&self) -> Result<(), MeshError> {
        let mut domain_edge: HashMap<VertexId, Point> = HashMap::new();
        for face_id in self
            .groups
            .get(&BoundaryKind::Inlet)
            .into_iter()
            .chain(self.groups.get(&BoundaryKind::Outlet))
            .chain(self.groups.get(&BoundaryKind::Wall))
            .flatten()
        {
            let face = self.faces[face_id.index()];
            domain_edge.insert(face.a, self.vertices[face.a.index()]);
            domain_edge.insert(face.b, self.vertices[face.b.index()]);
        }
        for face_id in self
            .groups
            .get(&BoundaryKind::Obstacle)
            .into_iter()
            .flatten()
        {
            let face = self.faces[face_id.index()];
            if let Some(&at) = domain_edge
                .get(&face.a)
                .or_else(|| domain_edge.get(&face.b))
            {
                return Err(MeshError::ObstacleTouchesBoundary { at });
            }
        }
        Ok(())
    }

    /// Calcule la distance associée à chaque face, une fois les voisins connus.
    fn finish_geometry(&mut self) {
        for face in &mut self.faces {
            let left = self.cells[face.left.index()].centroid;
            face.distance = match face.right {
                Side::Inner(right) => left.distance(self.cells[right.index()].centroid),
                Side::Boundary(_) => left.distance(face.midpoint),
            };
        }
    }

    /// Construit la connectivité cellule → faces au format CSR.
    fn build_cell_faces(&mut self) {
        let mut counts = vec![0u32; self.cells.len() + 1];
        for face in &self.faces {
            counts[face.left.index() + 1] += 1;
            if let Side::Inner(right) = face.right {
                counts[right.index() + 1] += 1;
            }
        }
        for i in 1..counts.len() {
            counts[i] += counts[i - 1];
        }
        let offsets = counts;

        let mut cursor = offsets.clone();
        let mut indices = vec![FaceId(0); offsets[self.cells.len()] as usize];
        for (i, face) in self.faces.iter().enumerate() {
            let fid = FaceId(i as u32);
            let slot = &mut cursor[face.left.index()];
            indices[*slot as usize] = fid;
            *slot += 1;
            if let Side::Inner(right) = face.right {
                let slot = &mut cursor[right.index()];
                indices[*slot as usize] = fid;
                *slot += 1;
            }
        }

        self.cell_face_offsets = offsets;
        self.cell_face_indices = indices;
    }
}

/// Sommets de la cellule `(row, col)` du masque, dans le sens direct.
///
/// Les quatre booléens, eux aussi dans le sens direct (bas, droite, haut, gauche),
/// disent si le voisin correspondant est une paroi d'obstacle. Quand exactement deux
/// parois perpendiculaires se rejoignent, le coin qu'elles encadrent est retiré : la
/// cellule devient un triangle et la marche d'escalier devient une facette à 45°.
///
/// Les coordonnées renvoyées sont celles de la grille de sommets, où `(row, col)` est
/// le coin **haut gauche** de la cellule `(row, col)`.
#[cfg_attr(not(feature = "step2"), allow(unused_variables))] // trou étape 2
fn cell_corners(
    row: usize,
    col: usize,
    down: bool,
    right: bool,
    up: bool,
    left: bool,
) -> Vec<(usize, usize)> {
    let bl = (row + 1, col);
    let br = (row + 1, col + 1);
    let tr = (row, col + 1);
    let tl = (row, col);
    // TODO-STEP:2 Renvoyer les quatre coins dans le sens direct (bl, br, tr, tl), en
    // retirant celui qui est coincé entre deux parois perpendiculaires
    // SOLUTION-BEGIN
    match (down, right, up, left) {
        (true, true, false, false) => vec![bl, tr, tl], // bas + droite : plus de br
        (false, true, true, false) => vec![bl, br, tl], // droite + haut : plus de tr
        (false, false, true, true) => vec![bl, br, tr], // haut + gauche : plus de tl
        (true, false, false, true) => vec![br, tr, tl], // gauche + bas : plus de bl
        _ => vec![bl, br, tr, tl],
    }
    // SOLUTION-END
}

/// Renvoie l'identifiant du sommet de la grille en `(row, col)`, en le créant au besoin.
///
/// La ligne 0 du masque est le haut du domaine : l'ordonnée décroît quand la ligne croît.
fn vertex_id(
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
    h: f64,
    vertices: &mut Vec<Point>,
    table: &mut [u32],
) -> VertexId {
    let k = row * (cols + 1) + col;
    if table[k] == u32::MAX {
        table[k] = vertices.len() as u32;
        vertices.push(Point::new(col as f64 * h, (rows - row) as f64 * h));
    }
    VertexId(table[k])
}

/// Apparie les arêtes des cellules pour en faire des faces.
///
/// Une arête vue une seule fois est une face de bord ; vue deux fois, une face interne ;
/// vue trois fois, un maillage cassé — d'où le `HashMap` d'arêtes en cours d'appariement.
#[cfg_attr(not(feature = "step2"), allow(unused_variables))] // trou étape 2
fn build_faces(cells: &[Cell], vertices: &[Point]) -> Result<Vec<Face>, MeshError> {
    // TODO-STEP:2 Apparier les arêtes : première rencontre ⇒ nouvelle face de bord
    // provisoire dont la normale est sortante de la cellule courante ; seconde
    // rencontre ⇒ la face devient interne (`Side::Inner`) ; troisième ⇒ `NonManifoldEdge`.
    // La clé d'une arête est la paire de sommets triée, pour que les deux cellules
    // adjacentes tombent bien sur la même entrée.
    // SOLUTION-BEGIN
    let mut faces: Vec<Face> = Vec::new();
    let mut seen: HashMap<(u32, u32), FaceId> = HashMap::new();

    for (i, cell) in cells.iter().enumerate() {
        let id = CellId(i as u32);
        let verts = cell.kind.vertices();
        for (k, &a) in verts.iter().enumerate() {
            let b = verts[(k + 1) % verts.len()];
            let key = if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) };
            match seen.get(&key) {
                Some(&fid) => match faces[fid.index()].right {
                    Side::Inner(_) => {
                        return Err(MeshError::NonManifoldEdge { a: key.0, b: key.1 })
                    }
                    Side::Boundary(_) => faces[fid.index()].right = Side::Inner(id),
                },
                None => {
                    let (pa, pb) = (vertices[a.index()], vertices[b.index()]);
                    let d = pb - pa;
                    faces.push(Face {
                        a,
                        b,
                        left: id,
                        // Nature provisoire : `classify_boundaries` tranchera pour de bon.
                        right: Side::Boundary(BoundaryKind::Wall),
                        normal: d.perp_cw().normalized(),
                        length: d.norm(),
                        midpoint: pa.midpoint(pb),
                        distance: 0.0,
                    });
                    seen.insert(key, FaceId((faces.len() - 1) as u32));
                }
            }
        }
    }
    Ok(faces)
    // SOLUTION-END
}

#[cfg(all(test, feature = "step2"))]
mod tests {
    use super::*;

    fn mesh_of(text: &str) -> Mesh {
        Mesh::from_mask(&Mask::parse(text).unwrap(), 1.0).unwrap()
    }

    #[test]
    fn mesh_of_a_two_by_two_mask() {
        let m = mesh_of("..\n..\n");
        assert_eq!(m.n_cells(), 4);
        assert_eq!(m.n_faces(), 12); // 4 cellules × 4 arêtes − 4 arêtes internes partagées
        assert!((m.total_area() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn chamfer_produces_a_triangle() {
        // le coin bas-droit de la cellule (0,0) est coincé entre deux parois
        let m = mesh_of(".#\n##\n");
        let (tri, quad) = m.shape_counts();
        assert_eq!((tri, quad), (1, 0));
        assert!((m.total_area() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn obstacle_touching_the_boundary_is_rejected() {
        // le mur haut n'est jamais atteint : cette obstacle-là commence dès la
        // rangée 0, donc partage un sommet avec lui.
        let m = mesh_of(".#\n##\n");
        assert!(m.check_obstacle_clear_of_boundary().is_err());
    }

    #[test]
    fn obstacle_clear_of_the_boundary_is_accepted() {
        // même obstacle en L, mais isolé du bord par une marge d'une cellule.
        let m = mesh_of("....\n..#.\n.##.\n....\n");
        assert!(m.check_obstacle_clear_of_boundary().is_ok());
    }

    #[test]
    fn inner_faces_are_shared_by_two_cells() {
        let m = mesh_of("...\n...\n");
        let inner = m.faces().iter().filter(|f| !f.is_boundary()).count();
        assert_eq!(inner, 7); // 2 lignes × 2 verticales + 3 horizontales
    }

    #[test]
    fn oriented_normals_sum_to_zero() {
        let m = mesh_of("...\n.#.\n...\n");
        for (i, cell) in m.cells().iter().enumerate() {
            let id = CellId(i as u32);
            let sum = m.cell_faces(id).iter().fold(Vec2::ZERO, |acc, &fid| {
                let f = m.face(fid);
                let sign = if f.left == id { 1.0 } else { -1.0 };
                acc + f.normal * (f.length * sign)
            });
            assert!(
                sum.norm() < 1e-12,
                "cellule {i} : {sum:?} (aire {})",
                cell.area
            );
        }
    }

    #[test]
    fn boundary_groups_are_classified() {
        let m = mesh_of("...\n.#.\n...\n");
        let g = m.groups();
        assert_eq!(g[&BoundaryKind::Inlet].len(), 3);
        assert_eq!(g[&BoundaryKind::Outlet].len(), 3);
        assert_eq!(g[&BoundaryKind::Wall].len(), 6);
        assert_eq!(g[&BoundaryKind::Obstacle].len(), 4);
    }
}
