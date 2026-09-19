//! Export au format VTK legacy ASCII (`UNSTRUCTURED_GRID`).
//!
//! Format volontairement daté : il tient en cinquante lignes, se lit dans un éditeur
//! de texte, et Paraview comme Tecplot l'ouvrent sans discuter.

use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::field::Field;
use crate::geom::Vec2;
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
pub fn write_vtk(path: impl AsRef<Path>, mesh: &Mesh, fields: &[(&str, &Field)]) -> io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    write_header(&mut w)?;
    write_dataset(&mut w, mesh, fields)?;
    w.flush()
}

/// Les quatre lignes d'en-tête, identiques dans tous les fichiers.
fn write_header(w: &mut impl Write) -> io::Result<()> {
    writeln!(w, "# vtk DataFile Version 3.0")?;
    writeln!(w, "wind-tunnel")?;
    writeln!(w, "ASCII")?;
    writeln!(w, "DATASET UNSTRUCTURED_GRID")
}

/// Le corps du fichier, en-tête exclu : géométrie, puis les champs aux cellules.
///
/// Écrit dans un `impl Write` plutôt que dans un fichier : c'est ce qui permet à
/// [`write_frame`] d'intercaler ses propres sections avant et après, sans dupliquer une
/// ligne de géométrie, et à un test d'écrire dans un `Vec<u8>`.
#[cfg_attr(not(feature = "step3"), allow(unused_variables))] // trou étape 3
fn write_dataset(w: &mut impl Write, mesh: &Mesh, fields: &[(&str, &Field)]) -> io::Result<()> {
    // TODO-STEP:3 Écrire les POINTS, les CELLS (précédées de leur nombre de sommets),
    // les CELL_TYPES, puis chaque champ en CELL_DATA / SCALARS. Les quatre lignes
    // d'en-tête sont déjà écrites par l'appelant.
    // Chaque `?` propage l'erreur d'écriture : rien n'est avalé en silence.
    // Tout flottant est enveloppé dans `VtkF64` — voir ce type pour la raison.
    // SOLUTION-BEGIN
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
    Ok(())
    // SOLUTION-END
}

/// Ce qu'une image de sortie contient, au-delà du seul traceur.
///
/// Les champs aux cellules et le temps servent à *lire* le résultat ; `psi`, aux sommets,
/// sert à lire l'**écoulement** : une isoligne de `ψ` est une ligne de courant exacte, donc
/// un simple filtre *Contour* dans ParaView trace les lignes de courant sans approximation
/// ni intégration de trajectoire.
pub struct Frame<'a> {
    /// Champs scalaires aux cellules, le traceur en tête.
    pub cells: &'a [(&'a str, &'a Field)],
    /// Champs vectoriels aux cellules — la vitesse reconstruite.
    pub vectors: &'a [(&'a str, &'a [Vec2])],
    /// Champs scalaires aux sommets — la fonction de courant.
    pub points: &'a [(&'a str, &'a [f64])],
    /// Date physique de l'image, en secondes.
    pub time: f64,
    /// Numéro du pas de temps correspondant.
    pub cycle: usize,
}

/// Écrit une image complète : géométrie, champs aux cellules, aux sommets, et la date.
///
/// La date est écrite en `FIELD FieldData`, sous trois noms : `TIME` et `CYCLE` (convention
/// VisIt), et `TimeValue` (celui que cherche le lecteur de séries de ParaView). Aucun n'est
/// garanti par le format lui-même — c'est une convention, pas une spécification — d'où les
/// trois. Si votre lecteur les ignore, la parade portable est de donner le même pas de temps
/// aux deux calculs (`--dt`) ou de sortir à cadence de temps fixe (`--every-dt`) : les images
/// de même rang portent alors la même date, et se comparent directement.
pub fn write_frame(path: impl AsRef<Path>, mesh: &Mesh, frame: &Frame) -> io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    write_header(&mut w)?;

    // Les données de champ du *jeu de données* se déclarent ici, entre `DATASET` et
    // `POINTS`. Plus bas, le lecteur legacy les rattache à la dernière section ouverte :
    // écrites après `POINT_DATA`, elles deviennent un tableau aux points, d'où
    // « Point array TIME with 1 components, only has 1 tuples but there are N points »,
    // puis « Attribute Mismatch », et ParaView renonce au fichier entier — zéro cellule
    // lue, série inouvrable. Constaté avec ParaView 6.1.1, corrigé, revérifié.
    writeln!(w, "FIELD FieldData 3")?;
    writeln!(w, "TIME 1 1 double")?;
    writeln!(w, "{}", VtkF64(frame.time))?;
    writeln!(w, "TimeValue 1 1 double")?;
    writeln!(w, "{}", VtkF64(frame.time))?;
    writeln!(w, "CYCLE 1 1 int")?;
    writeln!(w, "{}", frame.cycle)?;

    // Ouvre la section CELL_DATA, que les vecteurs ci-dessous prolongent.
    write_dataset(&mut w, mesh, frame.cells)?;

    for (name, values) in frame.vectors {
        writeln!(w, "VECTORS {name} double")?;
        for v in *values {
            writeln!(w, "{} {} 0", VtkF64(v.x), VtkF64(v.y))?;
        }
    }

    if !frame.points.is_empty() {
        writeln!(w, "POINT_DATA {}", mesh.n_vertices())?;
        for (name, values) in frame.points {
            writeln!(w, "SCALARS {name} double 1")?;
            writeln!(w, "LOOKUP_TABLE default")?;
            for value in *values {
                writeln!(w, "{}", VtkF64(*value))?;
            }
        }
    }

    w.flush()
}

/// Écrit la métadonnée de série que ParaView lit pour connaître la date de chaque image.
///
/// Format `.vtk.series` : un petit JSON listant les fichiers et leur date. C'est la voie
/// officielle, et la seule qui marche ici — le `.pvd`, plus connu, fait planter
/// `vtkPVDReader` sur du VTK legacy (voir `mpi/src/main.rs`), et la date inscrite dans le
/// fichier lui-même, en `FIELD FieldData`, est bien lue mais pas utilisée comme axe du
/// temps : sans ce fichier, ParaView numérote les images 0, 1, 2… Vérifié avec ParaView
/// 6.1.1.
///
/// Ouvrez `frames.vtk.series` plutôt que `frame_*.vtk` : deux calculs de pas de temps
/// différents se superposent alors sur le même axe, et l'animation montre le même instant
/// des deux côtés.
pub fn write_series(path: impl AsRef<Path>, frames: &[(String, f64)]) -> io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(w, "{{")?;
    writeln!(w, "  \"file-series-version\": \"1.0\",")?;
    writeln!(w, "  \"files\": [")?;
    for (i, (name, time)) in frames.iter().enumerate() {
        let comma = if i + 1 == frames.len() { "" } else { "," };
        writeln!(
            w,
            "    {{ \"name\": \"{name}\", \"time\": {} }}{comma}",
            VtkF64(*time)
        )?;
    }
    writeln!(w, "  ]")?;
    writeln!(w, "}}")?;
    w.flush()
}
