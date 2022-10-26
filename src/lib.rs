use boid::*;
use graphics::*;
use physics::*;
use unit::*;
use bevy_ecs::prelude::*;
use gdnative::{
    api::{RandomNumberGenerator, VisualServer},
    prelude::*,
};
use physics::spatial_structures::*;

mod boid;
mod graphics;
mod physics;
mod util;
mod unit;

#[derive(NativeClass)]
#[inherit(Node2D)]
pub struct ECSWorld {
    world: bevy_ecs::prelude::World,
    schedule: Schedule,
    schedule_logic: Schedule,
    canvas_item: Rid,
}

#[methods]
impl ECSWorld {
    fn new(base: &Node2D) -> Self {
        let mut schedule_physics = Schedule::default();
        schedule_physics.add_stage(
            "integrate_physics",
            SystemStage::parallel().with_system(physics_integrate),
        );
        schedule_physics.add_stage(
            "build_spatial_hash",
            SystemStage::parallel().with_system(build_spatial_hash_table),
        );
        schedule_physics.add_stage(
            "detect_collisions",
            SystemStage::parallel()
                .with_system(detect_collisions)
                .with_system(build_spatial_neighbors_cache),
        );

        let mut schedule_resolution = Schedule::default();
        schedule_resolution.add_stage(
            "resolve_collisions",
            SystemStage::parallel().with_system(resolve_collisions_iteration),
        );
        schedule_resolution.add_stage(
            "detect_collisions",
            SystemStage::parallel().with_system(detect_collisions),
        );
        let collisions_stage = CollisionStage {
            schedule: schedule_resolution,
            max_iterations: 8,
        };

        schedule_physics.add_stage("resolve_collisions", collisions_stage);

        let mut schedule_behavior = Schedule::default();
        //schedule_behavior.add_stage("build_flow_fields", SystemStage::parallel().with_system(build_flow_fields)); 
        schedule_behavior.add_stage(
            "boid_steer",
            SystemStage::parallel()
                .with_system(separation_boid)
                .with_system(stopping_boid),
        );
        schedule_behavior.add_stage(
            "boid_normalize",
            SystemStage::parallel().with_system(boid_apply_params),
        );

        let mut schedule_logic = Schedule::default();
        schedule_logic.add_stage("physics", schedule_physics);
        schedule_logic.add_stage("behavior", schedule_behavior);

        let mut schedule = Schedule::default();
        schedule.add_stage(
            "graphics",
            SystemStage::parallel().with_system(update_canvas_items),
        );

        let world = World::new();

        Self {
            world: world,
            schedule_logic: schedule_logic,
            schedule: schedule,
            canvas_item: base.get_canvas_item(),
        }
    }

    #[method]
    fn _ready(&mut self, #[base] _base: &Node2D) {
        let rand = RandomNumberGenerator::new();
        for _ in 0..500 {
            let random_position = Vector2 {
                x: rand.randi_range(100, 400) as f32,
                y: rand.randi_range(50, 450) as f32,
            };

            let radius = rand.randi_range(3, 6) as f32;
            let mass = radius * radius;
            let movespeed = rand.randf_range(2.5, 25.0) as f32;
            self.add_unit(1, random_position, Vector2::RIGHT, radius, mass, movespeed);
        }
        for _ in 0..500 {
            let random_position = Vector2 {
                x: rand.randi_range(500, 900) as f32,
                y: rand.randi_range(50, 450) as f32,
            };

            let radius = rand.randi_range(3, 6) as f32;
            let mass = radius * radius;
            let movespeed = rand.randf_range(2.5, 25.0) as f32;
            self.add_unit(2, random_position, Vector2::LEFT, radius, mass, movespeed);
        }
    }

    #[method]
    fn add_unit(
        &mut self,
        team_id: usize,
        position: Vector2,
        goal_direction: Vector2,
        radius: f32,
        mass: f32,
        movespeed: f32,
    ) -> u32 {
        let visual_server = unsafe { VisualServer::godot_singleton() };
        let canvas_item_rid = visual_server.canvas_item_create();
        unsafe {
            visual_server.canvas_item_set_parent(canvas_item_rid, self.canvas_item);
            visual_server.canvas_item_add_(
                canvas_item_rid,
                Vector2::ZERO,
                radius as f64,
                Color {
                    r: 0.,
                    g: 0.,
                    b: 1.,
                    a: 1.,
                },
            );
            visual_server.canvas_item_set_transform(
                canvas_item_rid,
                Transform2D::IDENTITY.translated(position),
            );
        };

        self.world
            .spawn()
            .insert(TeamAlignment{alignment: TeamValue::Team(team_id)})
            .insert(Position { pos: position })
            .insert(Velocity {
                v: goal_direction * movespeed,
            })
            .insert(Radius { r: radius })
            .insert(Mass(mass))
            .insert(BoidParams {
                max_force: 5. * mass,
                max_speed: movespeed,
            })
            .insert(SeparationBoid {
                avoidance_radius: 10.,
                multiplier: 3.,
            })
            .insert(StoppingBoid { multiplier: 1. })
            .insert(SpatialAwareness {
                radius: radius.max(10.),
            })
            .insert(AppliedForces(Vector2::ZERO))
            .insert(Renderable {
                canvas_item_rid: canvas_item_rid,
            })
            .id()
            .id()
    }

    #[method]
    fn _process(&mut self, delta: f32) {
        self.world.insert_resource(Delta { seconds: delta });
        self.schedule.run(&mut self.world);
    }

    #[method]
    fn _physics_process(&mut self, delta: f32) {
        self.world.insert_resource(DeltaPhysics { seconds: delta });
        let radii = [4., 16., 64., 256.];
        self.world
            .insert_resource(spatial_structures::SpatialNeighborsRadii(Box::new(radii)));
        self.schedule_logic.run(&mut self.world);
    }
}

fn init(handle: InitHandle) {
    handle.add_class::<ECSWorld>();
}

godot_init!(init);
