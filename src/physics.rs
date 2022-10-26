use crate::util::*;
use bevy_ecs::prelude::*;
use gdnative::prelude::*;
use spatial_structures::*;
use std::collections::{HashMap, HashSet};

pub mod spatial_structures;

pub struct CollisionStage {
    pub max_iterations: usize,
    pub schedule: Schedule,
}

impl Stage for CollisionStage {
    fn run(&mut self, world: &mut World) {
        let mut curr_run_count: usize = 0;
        for _ in 0..self.max_iterations {
            if !world.contains_resource::<CollisionInstanceVec>() {
                return;
            }
            if let Some(collisions) = world.get_resource::<CollisionInstanceVec>() {
                if collisions.0.len() <= curr_run_count {
                    return;
                }
                curr_run_count += 1;
                self.schedule.run_once(world);
            }
        }
    }
}

#[derive(Default)]
pub struct DeltaPhysics {
    pub seconds: f32,
}

#[derive(Component)]
pub struct Position {
    pub pos: Vector2,
}

#[derive(Component)]
pub struct Velocity {
    pub v: Vector2,
}

#[derive(Component)]
pub struct Radius {
    pub r: f32,
}

#[derive(Component)]
pub struct Mass(pub f32);

#[derive(Component)]
pub struct Elasticity(pub f32); // 0 to 1

#[derive(Component)]
pub struct AppliedForces(pub Vector2);

#[derive(Debug, Component)]
pub struct CollisionInstance {
    pub entity1: Entity,
    pub entity2: Entity,
    pub contact_normal: Vector2, // Vector2 from 1 to 2
    pub overlap: f32,
    pub mass_ratio: f32, // Ratio of 1:2
}

#[derive(Default)]
pub struct CollisionInstanceVec(pub Vec<CollisionInstance>);

pub fn physics_integrate(
    mut query: Query<(
        &mut Position,
        &mut Velocity,
        Option<&mut AppliedForces>,
        Option<&Mass>,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (mut position, mut velocity, force_option, mass_option) in &mut query {
        if let Some(mut force) = force_option {
            let force_vec = force.0;
            let mass = match mass_option {
                // Default mass = 1
                Some(mass_component) => 1., //mass_component.0,
                None => 1.,
            };

            velocity.v += force_vec / mass * delta.seconds;
            force.0 = Vector2::ZERO;
        }

        position.pos += velocity.v * delta.seconds;
    }
}

pub fn detect_collisions(
    mut commands: Commands,
    test_query: Query<(Entity, &Position, &Radius, &Mass)>,
    spatial: Res<SpatialHashTable>,
) {
    let mut collision_instances: Vec<CollisionInstance> = Vec::new();
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
                                overlap: overlap + 0.1,
                                mass_ratio: mass.0 / (mass.0 + mass2.0),
                            };
                            collision_instances.push(collision);
                        }
                    }
                }
            }
        }
    }
    if collision_instances.is_empty() {
        commands.remove_resource::<CollisionInstanceVec>();
    } else {
        commands.insert_resource(CollisionInstanceVec(collision_instances));
    }
}

pub fn resolve_collisions_iteration(
    mut resolve_query: Query<(&mut Position, &mut AppliedForces)>,
    collisions: Res<CollisionInstanceVec>,
) {
    for collision in &collisions.0 {
        if let Ok((mut position, mut _applied_forces)) = resolve_query.get_mut(collision.entity1) {
            position.pos += collision.contact_normal * (collision.overlap * collision.mass_ratio);
        }
        if let Ok((mut position, mut _applied_forces)) = resolve_query.get_mut(collision.entity2) {
            position.pos -=
                collision.contact_normal * (collision.overlap * (1.0 - collision.mass_ratio));
        }
    }
}

pub fn build_spatial_hash_table(
    mut commands: Commands,
    query: Query<(Entity, &Position, &Radius)>,
) {
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
pub fn get_all_spatial_hashes_from_circle(
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


pub fn build_spatial_neighbors_cache(
    mut commands: Commands,
    mut query: Query<(Entity, &Position, &Radius, &SpatialAwareness)>,
    inner_query: Query<(Entity, &Position, &Radius)>,
    radii: Res<SpatialNeighborsRadii>,
    spatial: Res<SpatialHashTable>,
) {
    let mut checked_ents: HashSet<Entity> = HashSet::new();
    let mut neighbor_cache: SpatialNeighborsCache = SpatialNeighborsCache::new(radii.0.clone());

    for (entity, position, radius, awareness) in query.iter_mut() {
        checked_ents.insert(entity);
        let mut checked_neighbors: HashSet<Entity> = checked_ents.clone();

        for spatial_hash in
            get_all_spatial_hashes_from_circle(position.pos, awareness.radius, spatial.cell_size)
                .iter()
        {
            let spatial_cell_group = spatial.table.get(&spatial_hash);
            if let Some(entity_group) = spatial_cell_group {
                for entity_test in entity_group.iter() {
                    // Don't collide with self or an already detected neighbor
                    if checked_neighbors.contains(entity_test) {
                        continue;
                    }
                    checked_neighbors.insert(*entity_test);
                    if let Ok((_, position_test, radius_test)) = inner_query.get(*entity_test) {
                        let distance_squared = true_distance_squared(
                            position.pos,
                            position_test.pos,
                            radius.r,
                            radius_test.r,
                        )
                        .max(0.0);
                        for i in 0..radii.0.len() {
                            let rad = radii.0.get(i).unwrap();
                            let rad_squared = rad * rad;
                            if distance_squared <= rad_squared {
                                // Add neighbor to entity1
                                //godot_print!("e1: {:?}, e2: {:?}", entity, *entity_test);
                                let neighbor_map = neighbor_cache.vec_of_maps.get_mut(i).unwrap();

                                if let Some(cached_neighbors) = neighbor_map.get_mut(&entity) {
                                    cached_neighbors.push((*entity_test, distance_squared.sqrt()));
                                } else {
                                    neighbor_map.insert(
                                        entity,
                                        vec![(*entity_test, distance_squared.sqrt())],
                                    );
                                }

                                // Add neighbor to entity_test
                                if let Some(cached_neighbors) = neighbor_map.get_mut(entity_test) {
                                    cached_neighbors.push((entity, distance_squared.sqrt()));
                                } else {
                                    neighbor_map.insert(
                                        *entity_test,
                                        vec![(entity, distance_squared.sqrt())],
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    commands.insert_resource(neighbor_cache);
}
