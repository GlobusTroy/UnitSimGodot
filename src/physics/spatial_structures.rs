use bevy_ecs::prelude::*;
use gdnative::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::{unit::TeamValue, util::{get_point_spatial_hash, true_distance_squared}};

use super::{Position, Radius};


#[derive(Default, Debug)]
pub struct FlowFieldsTowardsEnemies {
    pub map: HashMap<TeamValue, FlowField>
}

#[derive(Default, Debug)]
pub struct FlowField {
    pub table: HashMap<(i32, i32), Vector2>,
    pub cell_size: f32,
}

#[derive(Default, Debug)]
pub struct SpatialHashTable {
    pub table: HashMap<(i32, i32), Vec<Entity>>,
    pub cell_size: f32,
}

pub struct SpatialNeighborsRadii(pub Box<[f32]>);

#[derive(Component)]
pub struct SpatialAwareness {
    pub radius: f32,
}

#[derive(Debug)]
pub struct SpatialNeighborsCache {
    pub(super) radii: Box<[f32]>,
    pub(super) vec_of_maps: Vec<HashMap<Entity, Vec<(Entity, f32)>>>,
}

impl SpatialNeighborsCache {
    pub fn new(radii: Box<[f32]>) -> SpatialNeighborsCache {
        let mut vec_of_maps = Vec::new();
        for _ in 0..radii.len() {
            vec_of_maps.push(HashMap::new())
        }
        Self {
            radii: radii,
            vec_of_maps: vec_of_maps,
        }
    }

    pub fn get_neighbors(&self, entity: &Entity, distance: f32) -> Option<Vec<Entity>> {
        let index: usize = match self.radii.iter().position(|r| r >= &&distance) {
            Some(index) => index,
            None => self.radii.len() - 1,
        };

        let mut neighbor_vec = None;
        if let Some(neighbor_map) = self.vec_of_maps.get(index) {
            neighbor_vec = neighbor_map.get(entity);
        }

        if let Some(vec) = neighbor_vec {
            return Some(
                vec.clone()
                    .into_iter()
                    .filter_map(|(ent, dist)| if dist <= distance { Some(ent) } else { None })
                    .collect(),
            );
        } else {
            return None;
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