#![cfg(feature = "step10")]

//! Le rendu de l'étape 10 répartit les cellules entre threads et dessine dans une
//! image partagée, protégée par un `Mutex`. Deux rendus du même champ doivent produire
//! le même fichier au bit près : rien ne doit dépendre de l'ordre dans lequel les
//! threads obtiennent le verrou. Le `debug_assert_eq!` du suivi (`mpsc`), lui, vérifie
//! en passant qu'aucune cellule n'a été oubliée ou comptée deux fois.

use std::fs;

use wind_tunnel::field::Field;
use wind_tunnel::io::png::write_png;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;

fn mesh_of(mask: &str, refine: usize) -> Mesh {
    let mask = Mask::parse(mask).expect("masque invalide").refine(refine);
    Mesh::from_mask(&mask, 1.0).expect("maillage impossible")
}

fn render(mesh: &Mesh, name: &str) -> Vec<u8> {
    let field = Field::from_fn(mesh, |id| id.index() as f64);
    let path = std::env::temp_dir().join(name);
    write_png(&path, mesh, &field, 200, field.min_max()).expect("écriture impossible");
    let bytes = fs::read(&path).expect("relecture impossible");
    fs::remove_file(&path).ok();
    bytes
}

#[test]
fn rendering_is_deterministic_across_runs() {
    // Assez de cellules pour occuper plusieurs threads.
    let mesh = mesh_of(
        "..........\n\
         ...####...\n\
         ..######..\n\
         ...####...\n\
         ..........\n",
        3,
    );

    let a = render(&mesh, "wind-tunnel-test-render-a.png");
    let b = render(&mesh, "wind-tunnel-test-render-b.png");
    assert_eq!(
        a, b,
        "deux rendus du même champ diffèrent : une écriture a échappé au verrou"
    );
}

#[test]
fn rendering_a_handful_of_cells_still_works() {
    // Moins de cellules que de threads disponibles : certains threads n'ont rien à
    // faire, ce que le suivi (mpsc) doit encaisser sans bloquer ni paniquer.
    let mesh = mesh_of("...\n...\n", 0);

    let a = render(&mesh, "wind-tunnel-test-render-small-a.png");
    let b = render(&mesh, "wind-tunnel-test-render-small-b.png");
    assert_eq!(a, b);
}
