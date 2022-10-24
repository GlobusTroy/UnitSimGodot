use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use gdnative::{
    api::{RandomNumberGenerator, VisualServer},
    prelude::*,
};

#[derive(Component)]
struct Position {
    pos: Vector2,
}

#[derive(Component)]
struct Velocity {
    v: Vector2,
}

#[derive(Component)]
struct AppliedForces(Vector2);

#[derive(Component)]
struct MovementSpeed(f32);

#[derive(Component)]
/// Direction vector. Magnitude is irrelevant.
struct MovementDirection(Vector2);

#[derive(Component)]
struct Radius {
    r: f32,
}

#[derive(Component)]
struct Mass(f32);

#[derive(Component)]
struct Elasticity(f32); // 0 to 1

struct SeparationBoid {
    avoidance_radius: f32,
    avoidance_multiplier: f32,
}

struct FlockingBoid {
    flocking_radius: f32,
    flocking_multiplier: f32,
}

#[derive(Component)]
struct BoidAgent {
    inertial_force: f32,
    avoidance_radius: f32,
    avoidance_force: f32,
    flocking_radius: f32,
    flocking_force: f32,
}

// #[derive(Component)]
// struct BoidAgent {
//     max_speed: f32,
//     max_force: f32,
// }

#[derive(Component)]
struct Renderable {
    canvas_item_rid: Rid,
}

#[derive(Default)]
struct Delta {
    seconds: f32,
}

#[derive(Default, Debug)]
struct SpatialHashTable {
    table: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
}

#[derive(Default)]
struct DeltaPhysics {
    seconds: f32,
}

#[derive(Component, Debug)]
struct CollisionInstance {
    entity1: Entity,
    entity2: Entity,
    contact_normal: Vector2, // Vector2 from 1 to 2
    overlap: f32,
    mass_ratio: f32, // Ratio of 1:2
}

#[derive(Component)]
struct DisplaceTargetEffect {
    target_entity: Entity,
    displacement: Vector2,
}

#[derive(Component)]
struct PhysicsBody();

fn physics_integrate(mut query: Query<(&mut Position, &Velocity)>, delta: Res<DeltaPhysics>) {
    for (mut position, velocity) in &mut query {
        position.pos += velocity.v * delta.seconds;
    }
}

fn true_distance(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_to(pos2) - (rad1 + rad2)
}

fn true_distance_squared(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_squared_to(pos2) - ((rad1 + rad2) * (rad1 + rad2))
}

fn detect_and_get_collisions(
    collision_instances_out: &mut Vec<CollisionInstance>,
    test_query: &Query<(Entity, &Position, &Radius, &Mass)>,
    spatial: &SpatialHashTable,
) {
    let mut collisions: HashSet<(Entity, Entity)> = HashSet::new();
    for (entity, position, radius, mass) in test_query.iter() {
        for spatial_hash in
            get_all_spatial_hashes_from_circle(position.pos, radius.r, spatial.cell_size).iter()
        {
            let neighbor_group = spatial.table.get(&spatial_hash);
            if let Some(entity_group) = neighbor_group {
                for entity_test in entity_group.iter() {
                    // Don't collide with self or an already detected collision
                    if entity == *entity_test
                        || collisions.contains(&(*entity_test, entity))
                        || collisions.contains(&(entity, *entity_test))
                    {
                        continue;
                    }
                    if let Ok((_, position2, radius2, mass2)) = test_query.get(*entity_test) {
                        if true_distance_squared(position.pos, position2.pos, radius.r, radius2.r)
                            < 0.0
                        {
                            collisions.insert((entity, *entity_test));

                            let contact_normal = (position.pos - position2.pos).normalized();
                            let overlap =
                                -true_distance(position.pos, position2.pos, radius.r, radius2.r);

                            let collision = CollisionInstance {
                                entity1: entity,
                                entity2: *entity_test,
                                contact_normal: contact_normal,
                                overlap: overlap,
                                mass_ratio: mass.0 / (mass.0 + mass2.0),
                            };
                            collision_instances_out.push(collision);
                        }
                    }
                }
            }
        }
    }
}

fn resolve_collisions_iteration(
    resolve_query: &mut Query<(&mut Position, &mut AppliedForces)>,
    collisions: &Vec<CollisionInstance>,
) {
    for collision in collisions {
        if let Ok((mut position, mut _applied_forces)) = resolve_query.get_mut(collision.entity1) {
            position.pos += collision.contact_normal * (collision.overlap * collision.mass_ratio);
        }
        if let Ok((mut position, mut _applied_forces)) = resolve_query.get_mut(collision.entity2) {
            position.pos -=
                collision.contact_normal * (collision.overlap * (1.0 / collision.mass_ratio));
        }
    }
}

fn detect_and_resolve_collisions(
    calculate_query: Query<(Entity, &Position, &Radius, &Mass)>,
    mut resolve_query: Query<(&mut Position, &mut AppliedForces)>,
    spatial: ResMut<SpatialHashTable>,
) {
    let max_iterations = 10;
    let mut collisions: Vec<CollisionInstance> = Vec::new();
    for _ in 0..max_iterations {
        godot_print!("ello mate");
        detect_and_get_collisions(&mut collisions, &calculate_query, &spatial);
        godot_print!(" mate");
        if collisions.is_empty() {
            return;
        }
        resolve_collisions_iteration(&mut resolve_query, &collisions);
        godot_print!("hi ");
        collisions.clear();
    }
}

fn resolve_collisions(
    mut commands: Commands,
    collisions_query: Query<(Entity, &CollisionInstance)>,
    calculate_query: Query<(Entity, &Position, &Radius, &Mass)>,
) {
    for (entity, collision) in collisions_query.iter() {
        let CollisionInstance {
            entity1,
            entity2,
            contact_normal,
            overlap,
            mass_ratio,
        } = *collision;
        if let Ok((ent1, pos1, radius1, mass1)) = calculate_query.get(entity1) {
            if let Ok((ent2, pos2, radius2, mass2)) = calculate_query.get(entity2) {
                let (displacement1, displacement2) = get_displacement_vectors_from_collision(
                    &pos1.pos, &pos2.pos, &mass1.0, &mass2.0, &radius1.r, &radius2.r,
                );
                commands.spawn().insert(DisplaceTargetEffect {
                    target_entity: ent1,
                    displacement: displacement1,
                });
                commands.spawn().insert(DisplaceTargetEffect {
                    target_entity: ent2,
                    displacement: displacement2,
                });
            }
            commands.entity(entity).despawn();
        }
    }
}

fn resolve_displacement(
    mut commands: Commands,
    effect_query: Query<(Entity, &DisplaceTargetEffect)>,
    mut target_query: Query<&mut Position>,
) {
    for (entity, displace_effect) in effect_query.iter() {
        if let Ok(mut position) = target_query.get_mut(displace_effect.target_entity) {
            position.pos += displace_effect.displacement;
        }
        commands.entity(entity).despawn();
    }
}

fn build_spatial_hash_table(mut commands: Commands, query: Query<(Entity, &Position, &Radius)>) {
    let mut spatial = SpatialHashTable {
        table: HashMap::new(),
        cell_size: 36.,
    };
    for (entity, position, radius) in query.iter() {
        for spatial_hash in
            get_all_spatial_hashes_from_circle(position.pos, radius.r, spatial.cell_size)
        {
            let vec = spatial.table.get_mut(&spatial_hash);
            if let Some(collection) = vec {
                collection.push(entity);
            } else {
                spatial.table.insert(spatial_hash, vec![entity]);
            }
        }
    }
    commands.insert_resource(spatial);
}

/// Actually getting all cell intersections for AABB around circle
fn get_all_spatial_hashes_from_circle(
    position: Vector2,
    radius: f32,
    cell_size: f32,
) -> Vec<(i32, i32)> {
    let min_pos = position - Vector2::ONE * radius;
    let max_pos = position + Vector2::ONE * radius;
    let min_hash = get_point_spatial_hash(min_pos, cell_size);
    let max_hash = get_point_spatial_hash(max_pos, cell_size);
    let mut result: Vec<(i32, i32)> = Vec::new();
    for x in min_hash.0..=max_hash.0 {
        for y in min_hash.1..=max_hash.1 {
            result.push((x, y));
        }
    }
    return result;
}

fn get_point_spatial_hash(point: Vector2, cell_size: f32) -> (i32, i32) {
    ((point.x / cell_size) as i32, (point.y / cell_size) as i32)
}

/// Returns vectors to apply to position to resolve the collision.
/// Removes overlap and moves each entity based on the ratio of their masses.
/// First result should be applied to entity1, second to entity2
fn get_displacement_vectors_from_collision(
    pos1: &Vector2,
    pos2: &Vector2,
    mass1: &f32,
    mass2: &f32,
    radius1: &f32,
    radius2: &f32,
) -> (Vector2, Vector2) {
    let from_2_to_1 = *pos1 - *pos2;
    let combined_radius = radius1 + radius2;
    let overlap = combined_radius - from_2_to_1.length();
    if overlap > 0. {
        let mass_ratio = mass2 / (mass1 + mass2);
        let displacement_vector = from_2_to_1.normalized() * overlap;
        return (
            displacement_vector * mass_ratio,
            -displacement_vector * (1. - mass_ratio),
        );
    } else {
        return (Vector2::ZERO, Vector2::ZERO);
    }
}

fn goal_direction(mut query: Query<(&mut MovementDirection, &Velocity)>) {
    for (mut direction, velocity) in query.iter_mut() {
        if velocity.v.x > 0. {
            direction.0 = Vector2::RIGHT;
        } else if velocity.v.x < 0. {
            direction.0 = Vector2::LEFT;
        }
    }
}

fn boid_steering(
    mut query: Query<(Entity, &mut MovementDirection, &BoidAgent, &Position, &Mass)>,
    inner_query: Query<(Entity, &Position, &Radius, &Mass)>,
    spatial: Res<SpatialHashTable>,
) {
    for (entity, mut aim, boid, position, mass) in query.iter_mut() {
        let mut flock_center = position.pos * mass.0;
        let mut flock_mass = mass.0;
        let mut nearest_obstacle_distance = boid.avoidance_radius;
        let mut nearest_obstacle_pos: Option<Vector2> = None;

        let mut checked_neighbors: HashSet<Entity> = HashSet::new();
        checked_neighbors.insert(entity);

        let max_radius = boid.avoidance_radius.max(boid.flocking_radius);
        for spatial_hash in
            get_all_spatial_hashes_from_circle(position.pos, max_radius, spatial.cell_size).iter()
        {
            let neighbor_group = spatial.table.get(&spatial_hash);
            if let Some(entity_group) = neighbor_group {
                for entity_test in entity_group.iter() {
                    // Don't collide with self or an already detected neighbor
                    if checked_neighbors.contains(entity_test) {
                        continue;
                    }
                    checked_neighbors.insert(*entity_test);
                    if let Ok((_, position_test, radius_test, mass_test)) =
                        inner_query.get(*entity_test)
                    {
                        let distance = position.pos.distance_to(position_test.pos) - radius_test.r;
                        if distance < nearest_obstacle_distance {
                            nearest_obstacle_pos = Some(position_test.pos);
                            nearest_obstacle_distance = distance;
                        }
                        if distance < boid.flocking_radius {
                            flock_mass += mass_test.0;
                            flock_center += position_test.pos * mass_test.0;
                        }
                    }
                }
            }
        }

        if aim.0 != Vector2::ZERO {
            aim.0 = aim.0.normalized() * boid.inertial_force;
        }
        if let Some(obstacle_pos) = nearest_obstacle_pos {
            aim.0 += obstacle_pos.direction_to(position.pos) * boid.avoidance_force;
        }
        if flock_mass > mass.0 {
            flock_center = flock_center / flock_mass;
            aim.0 += position.pos.direction_to(flock_center) * boid.flocking_force;
        }
    }
}

fn unit_movement(mut query: Query<(&mut Velocity, &MovementSpeed, &MovementDirection)>) {
    for (mut velocity, speed, direction) in query.iter_mut() {
        velocity.v = direction.0.normalized() * speed.0;
    }
}

fn update_canvas_items(query: Query<(&Position, &Renderable)>) {
    for (position, renderable) in &query {
        unsafe {
            VisualServer::godot_singleton().canvas_item_set_transform(
                renderable.canvas_item_rid,
                Transform2D::IDENTITY.translated(position.pos),
            );
        }
    }
}

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
            "physics_integrate",
            SystemStage::parallel().with_system(physics_integrate),
        );
        schedule_physics.add_stage(
            "build_spatial_hash",
            SystemStage::parallel().with_system(build_spatial_hash_table),
        );
        schedule_physics.add_stage(
            "handle_collisions",
            SystemStage::parallel().with_system(detect_and_resolve_collisions),
        );

        let mut schedule_behavior = Schedule::default();
        schedule_behavior.add_stage("plan", SystemStage::parallel().with_system(goal_direction));
        schedule_behavior.add_stage("adjust", SystemStage::parallel().with_system(boid_steering));
        schedule_behavior.add_stage(
            "execute",
            SystemStage::parallel().with_system(unit_movement),
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
        for _ in 0..600 {
            let random_position = Vector2 {
                x: rand.randi_range(100, 900) as f32,
                y: rand.randi_range(50, 450) as f32,
            };

            let radius = rand.randi_range(3, 6) as f32;
            let mass = radius * radius;
            let movespeed = rand.randf_range(2.5, 25.0) as f32;
            self.add_unit(random_position, Vector2::RIGHT, radius, mass, movespeed);
        }
        for _ in 0..600 {
            let random_position = Vector2 {
                x: rand.randi_range(100, 900) as f32,
                y: rand.randi_range(50, 450) as f32,
            };

            let radius = rand.randi_range(3, 6) as f32;
            let mass = radius * radius;
            let movespeed = rand.randf_range(2.5, 25.0) as f32;
            self.add_unit(random_position, Vector2::LEFT, radius, mass, movespeed);
        }
    }

    #[method]
    fn add_unit(
        &mut self,
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
            visual_server.canvas_item_add_circle(
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
            .insert(Position { pos: position })
            .insert(Velocity { v: Vector2::ZERO })
            .insert(Radius { r: radius })
            .insert(Mass(mass))
            .insert(MovementSpeed(movespeed))
            .insert(MovementDirection(goal_direction))
            .insert(BoidAgent {
                inertial_force: 15.,
                avoidance_radius: radius + 2.,
                avoidance_force: 0.,
                flocking_radius: radius * 3.,
                flocking_force: 1.0,
            })
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
        self.schedule_logic.run(&mut self.world);
    }
}

fn init(handle: InitHandle) {
    handle.add_class::<ECSWorld>();
}

godot_init!(init);
