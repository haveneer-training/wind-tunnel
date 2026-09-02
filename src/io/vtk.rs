//! Export au format VTK legacy ASCII (`UNSTRUCTURED_GRID`).
//!
//! Format volontairement daté : il tient en cinquante lignes, se lit dans un éditeur
//! de texte, et Paraview comme Tecplot l'ouvrent sans discuter.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::field::Field;
use crate::mesh::Mesh;

/// Écrit le maillage et une liste de champs nommés.
pub fn write_vtk(path: impl AsRef<Path>, mesh: &Mesh, fields: &[(&str, &Field)]) -> io::Result<()> {
    // TODO-STEP:3 Écrire l'en-tête, les POINTS, les CELLS (précédées de leur nombre de
    // sommets), les CELL_TYPES, puis chaque champ en CELL_DATA / SCALARS.
    // Chaque `?` propage l'erreur d'écriture : rien n'est avalé en silence.
    // SOLUTION-BEGIN
    let mut w = BufWriter::new(File::create(path)?);

    writeln!(w, "# vtk DataFile Version 3.0")?;
    writeln!(w, "wind-tunnel")?;
    writeln!(w, "ASCII")?;
    writeln!(w, "DATASET UNSTRUCTURED_GRID")?;

    writeln!(w, "POINTS {} double", mesh.n_vertices())?;
    for p in mesh.vertices() {
        writeln!(w, "{} {} 0", p.x, p.y)?;
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
            writeln!(w, "{value}")?;
        }
    }

    w.flush()
    // SOLUTION-END
}
