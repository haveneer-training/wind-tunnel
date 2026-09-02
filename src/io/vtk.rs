//! Export au format VTK legacy ASCII (`UNSTRUCTURED_GRID`).
//!
//! Format volontairement daté : il tient en cinquante lignes, se lit dans un éditeur
//! de texte, et Paraview comme Tecplot l'ouvrent sans discuter.

use std::fmt;
#[cfg_attr(not(feature = "step3"), allow(unused_imports))] // trou étape 3
use std::fs::File;
#[cfg_attr(not(feature = "step3"), allow(unused_imports))] // trou étape 3
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::field::Field;
use crate::mesh::Mesh;

/// Un flottant sous une forme que le lecteur VTK legacy sait relire.
///
/// Deux précautions, dont une indispensable :
///
/// 1. **Les dénormaux sont écrasés à zéro.** `vtkDataReader` lit ses nombres par
///    `istream >> double`, qui pose `failbit` quand la conversion sous-déborde
///    (`strtod` renvoie `ERANGE` sur tout ce qui est sous [`f64::MIN_POSITIVE`]). Le flux
///    reste alors en échec : le lecteur abandonne le reste du champ et prend le nombre
///    suivant pour un mot-clé — « Error reading ascii data », puis « Unsupported cell
///    attribute type: 3.5e-323 », et Paraview n'affiche plus rien. Or un traceur qui
///    décroît exponentiellement loin de sa source descend jusque-là en quelques dizaines
///    de pas. À 1e-320 près, la concentration *est* nulle : l'écrire `0` ne perd rien.
/// 2. Au-delà des exposants usuels, on passe en `{:e}`. Ce n'est pas une question de
///    correction — un jeton de quatre cents caractères se relit très bien — mais `{}`
///    n'écrit jamais en notation scientifique, et `2.2e-303` occuperait trois cent
///    vingt-six caractères de zéros. Autant garder des fichiers lisibles et compacts.
///
/// C'est un type et non une fonction `fn(f64) -> String`, et ce n'est pas un détail :
/// un fichier de ce projet contient de l'ordre du million de nombres, soit un million
/// de `String` allouées puis jetées aussitôt. En implémentant [`fmt::Display`], le
/// nombre se formate directement dans le tampon de sortie, sans allocation
/// intermédiaire — `write!(w, "{}", VtkF64(x))` s'écrit pareil et n'alloue rien. C'est
/// l'usage normal de `Display` en Rust : *savoir s'écrire*, pas *fabriquer une chaîne*.
#[cfg_attr(not(feature = "step4"), allow(dead_code))] // collatéral du trou étape 3
struct VtkF64(f64);

impl fmt::Display for VtkF64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        let magnitude = value.abs();
        if magnitude < f64::MIN_POSITIVE {
            f.write_str("0")
        } else if !(1e-6..1e16).contains(&magnitude) {
            write!(f, "{value:e}")
        } else {
            write!(f, "{value}")
        }
    }
}

/// Écrit le maillage et une liste de champs nommés.
#[cfg_attr(not(feature = "step3"), allow(unused_variables))] // trou étape 3
pub fn write_vtk(path: impl AsRef<Path>, mesh: &Mesh, fields: &[(&str, &Field)]) -> io::Result<()> {
    // TODO-STEP:3 Écrire l'en-tête, les POINTS, les CELLS (précédées de leur nombre de
    // sommets), les CELL_TYPES, puis chaque champ en CELL_DATA / SCALARS.
    // Chaque `?` propage l'erreur d'écriture : rien n'est avalé en silence.
    // Tout flottant est enveloppé dans `VtkF64` — voir ce type pour la raison.
    // SOLUTION-BEGIN
    let mut w = BufWriter::new(File::create(path)?);

    writeln!(w, "# vtk DataFile Version 3.0")?;
    writeln!(w, "wind-tunnel")?;
    writeln!(w, "ASCII")?;
    writeln!(w, "DATASET UNSTRUCTURED_GRID")?;

    writeln!(w, "POINTS {} double", mesh.n_vertices())?;
    for p in mesh.vertices() {
        writeln!(w, "{} {} 0", VtkF64(p.x), VtkF64(p.y))?;
    }

    let entries: usize = mesh
        .cells()
        .iter()
        .map(|c| c.kind.vertices().len() + 1)
        .sum();
    writeln!(w, "CELLS {} {}", mesh.n_cells(), entries)?;
    for cell in mesh.cells() {
        let vertices = cell.kind.vertices();
        write!(w, "{}", vertices.len())?;
        for v in vertices {
            write!(w, " {}", v.index())?;
        }
        writeln!(w)?;
    }

    writeln!(w, "CELL_TYPES {}", mesh.n_cells())?;
    for cell in mesh.cells() {
        writeln!(w, "{}", cell.kind.vtk_code())?;
    }

    writeln!(w, "CELL_DATA {}", mesh.n_cells())?;
    for (name, field) in fields {
        writeln!(w, "SCALARS {name} double 1")?;
        writeln!(w, "LOOKUP_TABLE default")?;
        for value in field.iter() {
            writeln!(w, "{}", VtkF64(*value))?;
        }
    }

    w.flush()
    // SOLUTION-END
}
