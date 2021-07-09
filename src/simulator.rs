#![allow(clippy::enum_glob_use)]
use super::*;
use rand::prelude::*;
use std::collections::BinaryHeap;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SimEvent {
    MessageArrived(Entity),
    SpawnRandomNodes(u32),
    SpawnRandomMessages(u32),
}

pub struct Simulator {
    event_queue: EventQueue,
    pub message_log: VecDeque<(SimSeconds, String)>,
    rng: ThreadRng,
}
impl Simulator {
    pub fn new() -> Self {
        Self {
            event_queue: EventQueue::new(),
            message_log: VecDeque::new(),
            rng: rand::thread_rng(),
        }
    }
    pub fn schedule(&mut self, time_due: SimSeconds, event: SimEvent) {
        self.event_queue.push(TimedEvent {
            time: time_due,
            event,
        });
    }
    pub fn work_until(&mut self, world: &mut World, sim_time: SimSeconds) {
        while self
            .event_queue
            .peek()
            .filter(|event| event.time <= sim_time)
            .is_some()
        {
            let timed_event = self.event_queue.pop().unwrap();
            let sim_time_now = timed_event.time;
            let event = timed_event.event;
            self.apply_event(world, sim_time_now, event);
        }
    }
    pub fn spawn_random_node(&mut self, world: &mut World) {
        world.push(random_node(&mut self.rng));
    }
    pub fn spawn_message_between_random_nodes(
        &mut self,
        world: &mut World,
        start_time: SimSeconds,
    ) {
        let mut node_ents_query = <Entity>::query().filter(component::<UnderlayNodeId>());
        let node_ents: Vec<Entity> = node_ents_query.iter(world).copied().collect();
        let selected_node_ids: Vec<Entity> = node_ents
            .choose_multiple(&mut self.rng, 2)
            .copied()
            .collect();
        let source = selected_node_ids[0];
        let dest = selected_node_ids[1];
        self.spawn_message(world, start_time, source, dest)
    }
    pub fn spawn_message_to_random_node(
        &mut self,
        world: &mut World,
        start_time: SimSeconds,
        source: Entity,
    ) {
        let mut node_ents_query = <Entity>::query().filter(component::<UnderlayNodeId>());
        let node_ents: Vec<Entity> = node_ents_query
            .iter(world)
            .filter(|idx| **idx != source)
            .copied()
            .collect();
        if let Some(&dest) = node_ents.choose(&mut self.rng) {
            self.spawn_message(world, start_time, source, dest);
        }
    }
    pub fn spawn_message(
        &mut self,
        world: &mut World,
        start_time: SimSeconds,
        source: Entity,
        dest: Entity,
    ) {
        let &pos_source = world
            .entry(source)
            .unwrap()
            .get_component::<UnderlayPosition>()
            .unwrap();
        let &pos_dest = world
            .entry(dest)
            .unwrap()
            .get_component::<UnderlayPosition>()
            .unwrap();

        let flight_duration =
            f64::from(UnderlayPosition::distance(pos_source, pos_dest)) / FLIGHT_PER_SECOND;
        let end_time = start_time + flight_duration;

        let message_ent = world.push((
            UnderlayMessage { source, dest },
            TimeSpan {
                start: start_time,
                end: end_time,
            },
            UnderlayPath {
                start: pos_source,
                end: pos_dest,
            },
            pos_source,
        ));
        self.log(
            start_time,
            format!(
                "{}: Sending a message to {}",
                name(world, source),
                name(world, dest),
            ),
        );
        self.schedule(end_time, SimEvent::MessageArrived(message_ent));
    }
    fn apply_event(&mut self, world: &mut World, sim_time_now: SimSeconds, event: SimEvent) {
        use SimEvent::*;
        match event {
            MessageArrived(message_ent) => {
                let underlay_message = world
                    .entry_ref(message_ent)
                    .unwrap()
                    .into_component::<UnderlayMessage>()
                    .unwrap();
                self.log(
                    sim_time_now,
                    format!(
                        "{}: Got message from {}",
                        name(world, underlay_message.dest),
                        name(world, underlay_message.source),
                    ),
                );
                let new_source = underlay_message.dest;
                self.spawn_message_to_random_node(world, sim_time_now, new_source);
                world.remove(message_ent);
            }
            SpawnRandomNodes(count) => {
                for _ in 0..count {
                    self.spawn_random_node(world);
                }
            }
            SpawnRandomMessages(count) => {
                for _ in 0..count {
                    self.spawn_message_between_random_nodes(world, sim_time_now);
                }
            }
        }
    }
    fn log(&mut self, sim_time: SimSeconds, message: String) {
        self.message_log.push_front((sim_time, message));
        self.message_log.truncate(12);
    }
}

type EventQueue = BinaryHeap<TimedEvent>;

#[derive(Debug, Eq, PartialEq)]
struct TimedEvent {
    time: SimSeconds,
    event: SimEvent,
}
impl Ord for TimedEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Ordered by importance:
        // the most important event is the one with the lowest `time_due`...
        self.time.cmp(&other.time).reverse()
    }
}
impl PartialOrd for TimedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn random_node(rng: &mut impl Rng) -> (UnderlayNodeId, UnderlayPosition) {
    let name = format!("node{:#04}", rng.gen_range(0..10_000));
    let buffer_zone = 10.;
    (
        UnderlayNodeId(name),
        UnderlayPosition {
            x: rng.gen_range(buffer_zone..=(NET_MAX_X - buffer_zone)),
            y: rng.gen_range(buffer_zone..=(NET_MAX_Y - buffer_zone)),
        },
    )
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use wasm_bindgen_test::*;

//     wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
// }
