use bevy_ecs::prelude::{Component, Entity};
use gdnative::prelude::Vector2;
use std::collections::HashMap;

use crate::unit::TeamValue;


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
