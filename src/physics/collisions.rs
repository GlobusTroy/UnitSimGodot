use super::spatial_structures::*;
use super::*;
use crate::util::*;
use bevy_ecs::prelude::*;
use gdnative::prelude::*;
use std::collections::HashSet;

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

pub fn handle_terrain_collisions(
    mut test_query: Query<(&mut Position, &mut Velocity, &Radius)>,
    map: Res<TerrainMap>,
) {
    for (mut position, mut velocity, radius) in test_query.iter_mut() {
        for spatial_hash in
            get_all_spatial_hashes_from_circle(position.pos, radius.r, map.cell_size).iter()
        {
            if map.get_cell(*spatial_hash).pathable_mask == 0 {
                let terrain_pos = Vector2 {
                    x: map.cell_size * spatial_hash.0 as f32 + map.cell_size / 2.,
                    y: map.cell_size * spatial_hash.1 as f32 + map.cell_size / 2.,
                };
                let x_overlap;
                let y_overlap;

                let direction_away = terrain_pos.direction_to(position.pos);
                if direction_away.x >= 0. {
                    let right_wall = terrain_pos.x + map.cell_size / 2.;
                    x_overlap = right_wall - (position.pos.x - radius.r);
                } else {
                    let left_wall = terrain_pos.x - map.cell_size / 2.;
                    x_overlap = left_wall - (position.pos.x + radius.r);
                }

                if direction_away.y >= 0. {
                    let bot_wall = terrain_pos.y + map.cell_size / 2.;
                    y_overlap = bot_wall - (position.pos.y - radius.r);
                } else {
                    let top_wall = terrain_pos.x - map.cell_size / 2.;
                    y_overlap = top_wall - (position.pos.y + radius.r);
                }

                // if x_overlap.abs() < y_overlap.abs() {
                //     position.pos.x += x_overlap;
                //     //velocity.v.x = 0.0;
                // } else {
                //     position.pos.y += y_overlap;
                //     //velocity.v.y = 0.0;
                // }
            }
        }
    }
}

pub fn detect_collisions(
    mut commands: Commands,
    test_query: Query<(Entity, &Position, &Radius, &Mass)>,
    spatial: Res<SpatialHashTable>,
    map: Res<TerrainMap>,
) {
    let mut collision_instances: Vec<CollisionInstance> = Vec::new();
    let mut collisions: HashSet<(Entity, Entity)> = HashSet::new();
    let diameter_to_radius = 0.7;
    for (entity, position, radius, mass) in test_query.iter() {
        //TERRAIN collisions
        for spatial_hash in get_convolution_neighbor_cells(
            get_point_spatial_hash(position.pos, map.cell_size),
            3,
            map.max_bounds,
        )
        .iter()
        {
            if map.get_cell(*spatial_hash).pathable_mask == 0 {
                let terrain_pos = Vector2 {
                    x: map.cell_size * spatial_hash.0 as f32 + map.cell_size / 2.,
                    y: map.cell_size * spatial_hash.1 as f32 + map.cell_size / 2.,
                };
                let terrain_r = map.cell_size * diameter_to_radius;
                if true_distance_squared(position.pos, terrain_pos, radius.r, terrain_r) < 0.0 {
                    let contact_normal = (position.pos - terrain_pos).normalized();
                    let overlap = -true_distance(position.pos, terrain_pos, radius.r, terrain_r);

                    let collision = CollisionInstance {
                        entity1: entity,
                        entity2: entity,
                        contact_normal: contact_normal,
                        overlap: overlap,
                        mass_ratio: 1.0,
                    };
                    collision_instances.push(collision);
                }
            }
        }

        //Body on Body collisions
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
                                mass_ratio: mass2.0 / (mass.0 + mass2.0),
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

        if collision.entity1 == collision.entity2 {
            continue;
        }

        if let Ok((mut position, mut _applied_forces)) = resolve_query.get_mut(collision.entity2) {
            position.pos -=
                collision.contact_normal * (collision.overlap * (1.0 - collision.mass_ratio));
        }
    }
}
