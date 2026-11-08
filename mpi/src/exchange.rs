//! Les échanges MPI : tout ce qui fait circuler des octets, et rien d'autre.
//!
//! Qui possède quoi, qui envoie quoi à qui et dans quel ordre est déjà décidé par
//! [`wind_tunnel::decomposition`], qui se teste sans MPI. Il ne reste ici que le
//! transport — et c'est volontaire : la difficulté d'un code distribué est presque
//! toujours dans la première partie.

use mpi::collective::SystemOperation;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use wind_tunnel::decomposition::{Layout, pack_into, unpack};
use wind_tunnel::field::Field;

/// Les tampons d'un échange de halo, alloués une fois pour toutes.
///
/// Un échange a lieu avant **chaque** évaluation de résidu, soit une à deux fois par pas
/// de temps : allouer ses quatre tampons à chaque appel reviendrait à allouer et libérer
/// des milliers de fois des tableaux dont la taille ne change jamais. On les garde donc
/// dans une structure que le pilote crée une fois et prête à chaque échange.
///
/// C'est la même discipline que le tampon `work` de
/// [`Solver::run`](wind_tunnel::solver::Solver::run) : dans une boucle en temps, la
/// mémoire se réserve avant la boucle, pas dedans.
#[cfg_attr(not(feature = "step13"), allow(dead_code))] // collatéral du trou étape 11
pub struct Halo {
    to_left: Vec<f64>,
    to_right: Vec<f64>,
    from_left: Vec<f64>,
    from_right: Vec<f64>,
}

impl Halo {
    /// Réserve les tampons des quatre directions, d'après le découpage du rang.
    ///
    /// Leur taille ne dépend que du `Layout`, donc du maillage : elle est fixée pour
    /// toute la durée du calcul.
    pub fn new(layout: &Layout) -> Halo {
        Halo {
            to_left: Vec::with_capacity(layout.send_left.len()),
            to_right: Vec::with_capacity(layout.send_right.len()),
            from_left: vec![0.0; layout.recv_left.len()],
            from_right: vec![0.0; layout.recv_right.len()],
        }
    }

    /// Met à jour les cellules fantômes avec ce qu'en savent les rangs voisins.
    ///
    /// À appeler avant **chaque** évaluation de résidu : en RK2, deux fois par pas de
    /// temps.
    ///
    /// Poster les réceptions avant les envois n'est pas cosmétique : deux rangs qui
    /// s'envoient mutuellement un gros message avec des primitives bloquantes attendent
    /// chacun que l'autre reçoive, et le calcul s'arrête là. Les primitives immédiates
    /// rendent la main tout de suite, et `wait_all` est le seul point de rendez-vous.
    pub fn exchange(&mut self, world: &SimpleCommunicator, layout: &Layout, c: &mut Field) {
        pack_all(c, layout, &mut self.to_left, &mut self.to_right);
        transfer(
            world,
            layout,
            &self.to_left,
            &self.to_right,
            &mut self.from_left,
            &mut self.from_right,
        );
        unpack_all(c, layout, &self.from_left, &self.from_right);
    }
}

/// Empaquette les cellules à envoyer vers chaque côté.
fn pack_all(c: &Field, layout: &Layout, to_left: &mut Vec<f64>, to_right: &mut Vec<f64>) {
    // TODO-STEP:11 Remplir les deux tampons d'envoi à partir des listes de cellules
    // possédées correspondantes.
    // SOLUTION-BEGIN
    pack_into(c, &layout.send_left, to_left);
    pack_into(c, &layout.send_right, to_right);
    // SOLUTION-END
}

/// Poste et attend les réceptions et envois des voisins existants.
fn transfer(
    world: &SimpleCommunicator,
    layout: &Layout,
    to_left: &[f64],
    to_right: &[f64],
    from_left: &mut [f64],
    from_right: &mut [f64],
) {
    // TODO-STEP:11 Dans `multiple_scope`, poster les réceptions avant les envois,
    // puis attendre toutes les requêtes sans annoter leurs types génériques.
    // SOLUTION-BEGIN
    mpi::request::multiple_scope(4, |scope, requests| {
        let mut post = |rank, receive, send| {
            if let Some(rank) = rank {
                let peer = world.process_at_rank(rank as i32);
                requests.add(peer.immediate_receive_into(scope, receive));
                requests.add(peer.immediate_send(scope, send));
            }
        };
        post(layout.left(), from_left, to_left);
        post(layout.right(), from_right, to_right);
        requests.wait_all(&mut Vec::with_capacity(4));
    });
    // SOLUTION-END
}

/// Recopie dans le champ les valeurs reçues de chaque voisin.
fn unpack_all(c: &mut Field, layout: &Layout, from_left: &[f64], from_right: &[f64]) {
    // TODO-STEP:11 Recopier chaque tampon reçu dans les cellules fantômes du côté
    // correspondant.
    // SOLUTION-BEGIN
    unpack(c, &layout.recv_left, from_left);
    unpack(c, &layout.recv_right, from_right);
    // SOLUTION-END
}

/// Le plus petit pas de temps stable de tous les rangs.
///
/// Chaque rang ne voit que ses cellules, donc chacun trouve un pas de temps différent.
/// Les laisser avancer chacun au sien ne donnerait pas un résultat « un peu » faux : les
/// rangs ne seraient plus à la même date physique, et le champ échangé aux coupures
/// n'aurait plus de sens. `all_reduce` et non `reduce` : tous les rangs ont besoin de la
/// réponse, pas seulement le rang 0.
pub fn global_dt_max(world: &SimpleCommunicator, local: f64) -> f64 {
    // TODO-STEP:11 Réduire `local` sur tous les rangs avec l'opérateur MIN
    // (`all_reduce_into`, `SystemOperation::min()`), et renvoyer le résultat.
    // SOLUTION-BEGIN
    let mut global = 0.0;
    world.all_reduce_into(&local, &mut global, SystemOperation::min());
    global
    // SOLUTION-END
}

/// Somme d'une quantité sur tous les rangs.
pub fn global_sum(world: &SimpleCommunicator, local: f64) -> f64 {
    let mut global = 0.0;
    world.all_reduce_into(&local, &mut global, SystemOperation::sum());
    global
}

/// Extrema d'une quantité sur tous les rangs.
pub fn global_min_max(world: &SimpleCommunicator, local: (f64, f64)) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0, 0.0);
    world.all_reduce_into(&local.0, &mut lo, SystemOperation::min());
    world.all_reduce_into(&local.1, &mut hi, SystemOperation::max());
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wind_tunnel::mesh::CellId;

    fn test_layout() -> Layout {
        Layout {
            part: 0,
            parts: 1,
            extended: 0..3,
            owned: vec![CellId(0), CellId(1), CellId(2)],
            send_left: vec![CellId(0)],
            recv_left: vec![CellId(1)],
            send_right: vec![CellId(2)],
            recv_right: vec![CellId(2)],
        }
    }

    #[test]
    fn pack_and_unpack_helpers_use_the_layout_order() {
        let layout = test_layout();
        let mut field = Field::zeros(3);
        field[CellId(0)] = 2.0;
        field[CellId(1)] = 3.0;
        field[CellId(2)] = 5.0;
        let (mut to_left, mut to_right) = (Vec::new(), Vec::new());
        pack_all(&field, &layout, &mut to_left, &mut to_right);
        assert_eq!(to_left, [2.0]);
        assert_eq!(to_right, [5.0]);

        unpack_all(&mut field, &layout, &[7.0], &[11.0]);
        assert_eq!(field[CellId(1)], 7.0);
        assert_eq!(field[CellId(2)], 11.0);
    }

    #[test]
    fn transfer_with_no_neighbours_completes() {
        let universe = mpi::initialize().expect("MPI doit pouvoir être initialisé");
        let world = universe.world();
        let layout = Layout {
            part: 0,
            parts: 1,
            extended: 0..0,
            owned: Vec::new(),
            send_left: Vec::new(),
            recv_left: Vec::new(),
            send_right: Vec::new(),
            recv_right: Vec::new(),
        };
        transfer(&world, &layout, &[], &[], &mut [], &mut []);
    }
}
