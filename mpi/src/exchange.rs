//! Les échanges MPI : tout ce qui fait circuler des octets, et rien d'autre.
//!
//! Qui possède quoi, qui envoie quoi à qui et dans quel ordre est déjà décidé par
//! [`wind_tunnel::decomposition`], qui se teste sans MPI. Il ne reste ici que le
//! transport — et c'est volontaire : la difficulté d'un code distribué est presque
//! toujours dans la première partie.

use mpi::collective::SystemOperation;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;

use wind_tunnel::decomposition::{pack_into, unpack, Layout};
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
        // TODO-STEP:11 Empaqueter `send_left` et `send_right` dans `self.to_left` et
        // `self.to_right` avec `pack_into`, puis, dans un `mpi::request::multiple_scope`,
        // poster pour chaque voisin existant (`layout.left()`, `layout.right()`) une
        // réception `immediate_receive_into` PUIS un envoi `immediate_send`, attendre le
        // tout avec `wait_all`, et recopier les tampons reçus dans le champ avec `unpack`.
        // SOLUTION-BEGIN
        pack_into(c, &layout.send_left, &mut self.to_left);
        pack_into(c, &layout.send_right, &mut self.to_right);

        // Les emprunts sont pris ici en une fois : la fermeture ci-dessous emprunte
        // `from_left`/`from_right` en écriture et `to_left`/`to_right` en lecture.
        // Avec du Rust ≥ 2021, l'emprunt partiel de champs de self fonctionne
        // et les intermédiaires ne sont pas indispensables.
        let (to_left, to_right) = (&self.to_left, &self.to_right);
        let (from_left, from_right) = (&mut self.from_left, &mut self.from_right);

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
            // Les messages d'un rang viennent de deux voisins distincts : la source
            // suffit à les distinguer, aucune étiquette n'est nécessaire.
            let mut completed = Vec::with_capacity(4);
            requests.wait_all(&mut completed);
        });

        unpack(c, &layout.recv_left, &self.from_left);
        unpack(c, &layout.recv_right, &self.from_right);
        // SOLUTION-END
    }
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
