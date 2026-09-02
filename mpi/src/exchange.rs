//! Les échanges MPI : tout ce qui fait circuler des octets, et rien d'autre.
//!
//! Qui possède quoi, qui envoie quoi à qui et dans quel ordre est déjà décidé par
//! [`wind_tunnel::decomposition`], qui se teste sans MPI. Il ne reste ici que le
//! transport — et c'est volontaire : la difficulté d'un code distribué est presque
//! toujours dans la première partie.

use mpi::collective::SystemOperation;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use wind_tunnel::decomposition::{pack, unpack, Layout};
use wind_tunnel::field::Field;

/// Met à jour les cellules fantômes avec ce qu'en savent les rangs voisins.
///
/// À appeler avant **chaque** évaluation de résidu : en RK2, deux fois par pas de temps.
///
/// Poster les réceptions avant les envois n'est pas cosmétique : deux rangs qui
/// s'envoient mutuellement un gros message avec des primitives bloquantes attendent
/// chacun que l'autre reçoive, et le calcul s'arrête là. Les primitives immédiates
/// rendent la main tout de suite, et `wait_all` est le seul point de rendez-vous.
pub fn exchange_halo(world: &SimpleCommunicator, layout: &Layout, c: &mut Field) {
    // TODO-STEP:11 Empaqueter `send_left` et `send_right` avec `pack`, préparer deux
    // tampons de réception, puis, dans un `mpi::request::multiple_scope`, poster pour
    // chaque voisin existant (`layout.left()`, `layout.right()`) une réception
    // `immediate_receive_into` PUIS un envoi `immediate_send`, attendre le tout avec
    // `wait_all`, et recopier les tampons reçus dans le champ avec `unpack`.
    // SOLUTION-BEGIN
    let to_left = pack(c, &layout.send_left);
    let to_right = pack(c, &layout.send_right);
    let mut from_left = vec![0.0; layout.recv_left.len()];
    let mut from_right = vec![0.0; layout.recv_right.len()];

    mpi::request::multiple_scope(4, |scope, requests| {
        if let Some(left) = layout.left() {
            let peer = world.process_at_rank(left as i32);
            requests.add(peer.immediate_receive_into(scope, &mut from_left[..]));
            requests.add(peer.immediate_send(scope, &to_left[..]));
        }
        if let Some(right) = layout.right() {
            let peer = world.process_at_rank(right as i32);
            requests.add(peer.immediate_receive_into(scope, &mut from_right[..]));
            requests.add(peer.immediate_send(scope, &to_right[..]));
        }
        // Les messages d'un rang viennent de deux voisins distincts : la source suffit
        // à les distinguer, aucune étiquette n'est nécessaire.
        let mut completed = Vec::new();
        requests.wait_all(&mut completed);
    });

    unpack(c, &layout.recv_left, &from_left);
    unpack(c, &layout.recv_right, &from_right);
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
