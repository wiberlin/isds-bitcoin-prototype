use super::*;

use std::cmp;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PeerSetUpdate {
    PeerAdded(Entity),
    PeerRemoved(Entity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddPeer(pub Entity, pub Entity);
impl Command for AddPeer {
    fn execute(&self, sim: &mut Simulation) -> Result<(), Box<dyn Error>> {
        add_peer(sim, self.0, self.1);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovePeer(pub Entity, pub Entity);
impl Command for RemovePeer {
    fn execute(&self, sim: &mut Simulation) -> Result<(), Box<dyn Error>> {
        remove_peer(sim, self.0, self.1);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MakeDelaunayNetwork;
impl Command for MakeDelaunayNetwork {
    fn execute(&self, sim: &mut Simulation) -> Result<(), Box<dyn Error>> {
        make_delaunay_network(sim);
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PeerSet {
    peers: BTreeSet<Entity>,
    last_update: SimSeconds, // for helping the UI know when to redraw
}
impl PeerSet {
    /// Only useful for tests really.
    pub fn default_from(peers: impl IntoIterator<Item = Entity>) -> Self {
        Self {
            peers: peers.into_iter().collect(),
            last_update: Default::default(),
        }
    }
    pub fn iter(&self) -> std::collections::btree_set::Iter<Entity> {
        self.peers.iter()
    }
    pub fn insert(&mut self, peer: Entity, now: SimSeconds) -> bool {
        if self.peers.insert(peer) {
            self.last_update = now;
            true
        } else {
            false
        }
    }
    pub fn remove(&mut self, peer: &Entity, now: SimSeconds) -> bool {
        if self.peers.remove(peer) {
            self.last_update = now;
            true
        } else {
            false
        }
    }
    pub fn contains(&self, peer: &Entity) -> bool {
        self.peers.contains(peer)
    }
    pub fn len(&self) -> usize {
        self.peers.len()
    }
    pub fn last_update(&self) -> SimSeconds {
        self.last_update
    }
}
impl IntoIterator for PeerSet {
    type Item = Entity;
    type IntoIter = std::collections::btree_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.peers.into_iter()
    }
}

fn make_delaunay_network(sim: &mut Simulation) {
    use delaunator::{triangulate, Point};
    let (nodes, points): (Vec<Entity>, Vec<Point>) = sim
        .world
        .query_mut::<(&UnderlayNodeName, &UnderlayPosition)>()
        .into_iter()
        .map(|(id, (_, pos))| {
            (
                id,
                Point {
                    x: pos.x as f64,
                    y: pos.y as f64,
                },
            )
        })
        .unzip();
    for &node in nodes.iter() {
        *peers(sim, node) = PeerSet::default();
    }
    let triangles = triangulate(&points)
        .expect("No triangulation exists.")
        .triangles;
    assert!(triangles.len() % 3 == 0);
    for i in (0..triangles.len()).step_by(3) {
        let node1 = nodes[triangles[i]];
        let node2 = nodes[triangles[i + 1]];
        let node3 = nodes[triangles[i + 2]];
        add_peer(sim, node1, node2);
        add_peer(sim, node1, node3);
        add_peer(sim, node2, node1);
        add_peer(sim, node2, node3);
        add_peer(sim, node3, node1);
        add_peer(sim, node3, node2);
    }
}

pub fn add_random_nodes_as_peers(
    sim: &mut Simulation,
    node: Entity,
    new_peers_min: usize,
    new_peers_max: usize,
) {
    let mut candidates = sim.all_other_nodes(node);
    let peers = peers(sim, node).clone();
    candidates.retain(|id| !peers.contains(id));

    let new_peers_min = cmp::min(new_peers_min, candidates.len());
    let new_peers_max = cmp::min(new_peers_max, candidates.len());
    let number_of_new_peers = sim.rng.gen_range(new_peers_min..new_peers_max);

    let new_peers = candidates.choose_multiple(&mut sim.rng, number_of_new_peers);
    for &peer in new_peers {
        add_peer(sim, node, peer);
    }
}

pub fn add_peer(sim: &mut Simulation, node: Entity, peer: Entity) {
    let now = sim.time.now();
    peers(sim, node).insert(peer, now);
    sim.schedule_now(Event::Node(
        node,
        NodeEvent::PeerSetChanged(PeerSetUpdate::PeerAdded(peer)),
    ));
}

pub fn remove_peer(sim: &mut Simulation, node: Entity, peer: Entity) {
    let now = sim.time.now();
    peers(sim, node).remove(&peer, now);
    sim.schedule_now(Event::Node(
        node,
        NodeEvent::PeerSetChanged(PeerSetUpdate::PeerRemoved(peer)),
    ));
}

pub fn peers(sim: &mut Simulation, node: Entity) -> hecs::RefMut<PeerSet> {
    if sim.world.get_mut::<PeerSet>(node).is_err() {
        let peers = PeerSet::default();
        sim.world.insert_one(node, peers).unwrap();
    }
    sim.world.get_mut::<PeerSet>(node).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn add_peer_adds_peer() {
        let mut sim = Simulation::new();
        let node1 = sim.spawn_random_node();
        let node2 = sim.spawn_random_node();
        add_peer(&mut sim, node1, node2);

        let expected = PeerSet {
            peers: vec![node2].into_iter().collect(),
            last_update: Default::default(),
        };
        let actual = (*sim.world.get::<PeerSet>(node1).unwrap()).clone();

        assert_eq!(expected, actual);
    }

    #[wasm_bindgen_test]
    fn add_random_other_nodes_as_peers_adds_peers() {
        let mut sim = Simulation::new();
        let node1 = sim.spawn_random_node();
        sim.spawn_random_node();
        sim.spawn_random_node();
        sim.spawn_random_node();
        sim.spawn_random_node();

        add_random_nodes_as_peers(&mut sim, node1, 2, 3);

        let peers = peers(&mut sim, node1);
        let actual = peers.len();
        let expected_min = 2;
        let expected_max = 3;

        assert!(expected_min <= actual);
        assert!(actual <= expected_max);
        assert!(!peers.contains(&node1));
    }
}
