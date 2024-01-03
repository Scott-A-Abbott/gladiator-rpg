use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node)]
struct Ecs {
    pub world: World,
    pub schedule_order: ScheduleOrder,

    #[base]
    node: Base<Node>,
}

#[godot_api]
impl INode for Ecs {
    fn init(node: Base<Node>) -> Self {
        let mut world = Self::world();

        let ecs_node = EcsNode(node.clone());
        world.insert_non_send_resource(ecs_node);

        let input = InputSingleton(Input::singleton());
        world.insert_non_send_resource(input);

        Self {
            world,
            schedule_order: ScheduleOrder::default(),
            node,
        }

        // Use configure functions for separate modules?
        // main_world::configure(&mut ecs);
        // combat::configure(&mut ecs);
        // pause_ui::configure(&mut ecs);

        // ecs
    }

    //## remove if unused
    fn ready(&mut self) {}

    fn process(&mut self, delta: f64) {
        let mut process_delta = self.world.resource_mut();
        *process_delta = ProcessDelta(delta);

        for label in self.schedule_order.process.iter() {
            self.world.try_run_schedule(*label).ok();
        }
    }

    fn physics_process(&mut self, delta: f64) {
        let mut physics_delta = self.world.resource_mut();
        *physics_delta = PhysicsDelta(delta);

        for label in self.schedule_order.physics.iter() {
            self.world.try_run_schedule(*label).ok();
        }
    }
}

impl Ecs {
    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<ProcessDelta>();
        world.init_resource::<PhysicsDelta>();
        world.insert_resource(Self::schedules());

        world
    }

    fn schedules() -> Schedules {
        let mut schedules = Schedules::new();

        let mut process = Schedule::new(Process);
        process.add_systems(
            |input_res: NonSend<InputSingleton>, ecs_res: NonSend<EcsNode>| {
                let input = &input_res.0;

                let escape = godot::engine::global::Key::KEY_ESCAPE;
                if input.is_key_pressed(escape) {
                    godot_print!("Quitting!");

                    let ecs = &ecs_res.0;
                    let mut tree = ecs.get_tree().unwrap();
                    tree.quit();
                }
            },
        );
        schedules.insert(process);

        let mut post_physics = Schedule::new(PostPhysics);
        // Signal to clear events after main physics systems have had a chance to process them
        post_physics.add_systems(event_queue_update_system);
        schedules.insert(post_physics);

        schedules
    }

    pub fn add_event<T>(&mut self) -> &mut Self
    where
        T: Event,
    {
        if !self.world.contains_resource::<Events<T>>() {
            self.world.init_resource::<Events<T>>();

            self.add_systems(
                PreProcess,
                event_update_system::<T>.run_if(bevy_ecs::event::event_update_condition::<T>),
            );
        }

        self
    }

    pub fn add_systems<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        let schedule = schedule.intern();

        let mut schedules = self.world.resource_mut::<Schedules>();
        if let Some(schedule) = schedules.get_mut(schedule) {
            schedule.add_systems(systems);
        } else {
            let mut new_schedule = Schedule::new(schedule);
            new_schedule.add_systems(systems);
            schedules.insert(new_schedule);
        }

        self
    }

    pub fn init_state<S>(&mut self) -> &mut Self
    where
        S: States,
    {
        if !self.world.contains_resource::<State<S>>() {
            use bevy_ecs::schedule::run_enter_schedule;

            self.world.init_resource::<State<S>>();
            self.world.init_resource::<NextState<S>>();

            let state_systems = (
                run_enter_schedule::<S>.run_if(run_once()),
                apply_state_transition::<S>,
            );

            self.add_systems(StateTransition, state_systems.chain());
        }

        self
    }
}

struct ScheduleOrder {
    process: Vec<bevy_ecs::schedule::InternedScheduleLabel>,
    physics: Vec<bevy_ecs::schedule::InternedScheduleLabel>,
}
impl Default for ScheduleOrder {
    fn default() -> Self {
        Self {
            process: vec![
                StateTransition.intern(),
                PreProcess.intern(),
                Process.intern(),
            ],
            physics: vec![Physics.intern(), PostPhysics.intern()],
        }
    }
}

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct StateTransition;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct PreProcess;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct Process;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct Physics;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct PostPhysics;

#[derive(Resource, Default)]
struct ProcessDelta(pub f64);

#[derive(Resource, Default)]
struct PhysicsDelta(pub f64);

// Non Send Resources
struct EcsNode(Gd<Node>);
struct InputSingleton(Gd<Input>);

//Reimplemented from bevy_ecs, because the bool is private
#[derive(Resource, Default)]
pub struct EventUpdateSignal(bool);

/// A system that queues a call to [`Events::update`].
pub fn event_queue_update_system(signal: Option<ResMut<EventUpdateSignal>>) {
    if let Some(mut s) = signal {
        s.0 = true;
    }
}

/// A system that calls [`Events::update`].
pub fn event_update_system<T: Event>(
    signal: Option<ResMut<EventUpdateSignal>>,
    mut events: ResMut<Events<T>>,
) {
    if let Some(mut s) = signal {
        // If we haven't got a signal to update the events, but we *could* get such a signal
        // return early and update the events later.
        if !std::mem::replace(&mut s.0, false) {
            return;
        }
    }

    events.update();
}
