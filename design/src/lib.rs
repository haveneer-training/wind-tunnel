//! Exercice de conception : les types de la connectivité, écrits de zéro.
//!
//! Partout ailleurs dans le fil rouge, les types sont donnés et vous en remplissez les
//! fonctions. Ici, c'est l'inverse : **ce fichier est vide**, et c'est à vous d'y écrire
//! les définitions. Le compilateur est votre seul guide, comme il le sera le jour où
//! vous concevrez vos propres structures de données.
//!
//! Le sujet est une miniature de ce que fait `src/mesh.rs` : décrire qui est de chaque
//! côté d'une face. Rien d'autre du projet n'en dépend — ce crate ne dépend de rien et
//! rien ne dépend de lui — donc aucune forme ne vous est imposée par ailleurs.
//!
//! ```shell
//! cargo test -p wind-tunnel-design
//! ```
//!
//! Les tests, eux, sont donnés : ils sont dans `design/tests/connectivity.rs`, ils
//! fixent les noms, et **ils ne compilent pas** tant que les définitions manquent — la
//! première erreur les nomme toutes d'un coup, et la liste se vide à mesure que vous
//! écrivez. C'est l'état normal de qui conçoit un type, et il est volontairement
//! cantonné à ce crate : le reste du projet continue de compiler et de tester.
//!
//! L'énoncé complet — le contrat que ces types doivent remplir, et ce qu'il y a à
//! remarquer une fois que c'est écrit — est dans
//! `docs/etapes/etape-02-conception.md`.

// TODO-STEP:2 (conception, facultatif) Écrire ici les types de la connectivité :
// `CellId` et `VertexId`, `BoundaryKind`, `Side`, `Face`, les deux méthodes de `Face` et
// la fonction `neighbours`. Le contrat détaillé est dans
// `docs/etapes/etape-02-conception.md` ; les tests de `tests/connectivity.rs` disent le
// reste. À faire **avant** de lire `src/mesh.rs`, puis à comparer avec lui.
// SOLUTION-BEGIN vide

/// Identifiant d'une cellule.
///
/// Un type à part entière autour d'un `u32`, et non un alias : c'est ce qui rend
/// impossible de passer un numéro de sommet là où l'on attend une cellule. Le coût à
/// l'exécution est nul — après compilation, c'est un `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellId(pub u32);

impl CellId {
    /// L'indice correspondant, pour aller lire un tableau.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Identifiant d'un sommet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VertexId(pub u32);

impl VertexId {
    /// L'indice correspondant, pour aller lire un tableau.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// La nature d'une frontière du domaine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryKind {
    /// Une paroi imperméable.
    Wall,
    /// L'entrée du domaine.
    Inlet,
    /// La sortie du domaine.
    Outlet,
}

/// Ce qu'il y a de l'autre côté d'une face : une cellule, ou une frontière.
///
/// Jamais les deux, jamais ni l'un ni l'autre — et c'est l'énumération qui le garantit,
/// plutôt qu'un `Option<CellId>` doublé d'un drapeau que rien n'obligerait à tenir
/// cohérent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Une cellule voisine.
    Inner(CellId),
    /// Une condition aux limites.
    Boundary(BoundaryKind),
}

/// Une face, orientée de `a` vers `b`, avec `left` d'un côté et `right` de l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    /// Premier sommet.
    pub a: VertexId,
    /// Second sommet.
    pub b: VertexId,
    /// La cellule de gauche : il y en a toujours une.
    pub left: CellId,
    /// Ce qu'il y a à droite.
    pub right: Side,
}

impl Face {
    /// La face est-elle sur une frontière du domaine ?
    pub fn is_boundary(&self) -> bool {
        matches!(self.right, Side::Boundary(_))
    }

    /// La cellule de l'autre côté, vue depuis `from`.
    ///
    /// `None` dans trois cas qui n'en font qu'un : la face est sur une frontière, ou
    /// `from` n'est tout simplement pas une des cellules de cette face. Le type dit
    /// qu'il peut ne pas y avoir de réponse ; l'appelant ne peut pas l'oublier.
    pub fn neighbour(&self, from: CellId) -> Option<CellId> {
        match self.right {
            Side::Inner(right) if from == self.left => Some(right),
            Side::Inner(right) if from == right => Some(self.left),
            _ => None,
        }
    }
}

/// Les voisines d'une cellule, dans l'ordre des faces données.
///
/// `faces` est **emprunté** : la fonction le lit sans s'en approprier, et l'appelant
/// s'en sert encore après. Seul le `Vec` rendu est à elle.
pub fn neighbours(faces: &[Face], cell: CellId) -> Vec<CellId> {
    faces
        .iter()
        .filter_map(|face| face.neighbour(cell))
        .collect()
}
// SOLUTION-END
