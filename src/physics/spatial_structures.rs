use bevy_ecs::prelude::*;
use gdnative::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    unit::{TeamAlignment, TeamValue},
    util::*,
    Clock,
};

use super::{Position, Radius};

#[derive(Debug, Clone, Copy, Default)]
pub struct TerrainCell {
    pub pathable_mask: usize,
    pub movement_cost: f32,
}

#[derive(Default, Debug, Clone)]
pub struct TerrainMap {
    pub map: HashMap<SpatialHashCell, TerrainCell>,
    pub max_bounds: SpatialHashCell,
    pub default_cell: TerrainCell,
    pub out_of_bounds_cell: TerrainCell,
    pub cell_size: f32,
}

impl TerrainMap {
    pub fn get_cell(&self, coords: SpatialHashCell) -> &TerrainCell {
        if coords.0 < 0
            || coords.1 < 0
            || coords.0 >= self.max_bounds.0
            || coords.1 >= self.max_bounds.1
        {
            return &self.out_of_bounds_cell;
        }

        match self.map.get(&coords) {
            Some(terrain_cell) => &terrain_cell,
            None => &self.default_cell,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Default)]
pub struct SpatialHashCell(pub i32, pub i32);

#[derive(Default, Debug)]
pub struct FlowFieldsTowardsEnemies {
    pub map: HashMap<SpatialHashCell, HashMap<TeamValue, Vector2>>,
    pub cell_size: f32,
}

#[derive(Default, Debug)]
pub struct FlowField {
    pub table: HashMap<SpatialHashCell, Vector2>,
    pub cell_size: f32,
}

#[derive(Default, Debug)]
pub struct SpatialHashTable {
    pub table: HashMap<SpatialHashCell, Vec<Entity>>,
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
) -> Vec<SpatialHashCell> {
    let min_pos = position - Vector2::ONE * radius;
    let max_pos = position + Vector2::ONE * radius;
    let min_hash = get_point_spatial_hash(min_pos, cell_size);
    let max_hash = get_point_spatial_hash(max_pos, cell_size);
    let mut result: Vec<SpatialHashCell> = Vec::new();
    for x in min_hash.0..=max_hash.0 {
        for y in min_hash.1..=max_hash.1 {
            result.push(SpatialHashCell(x, y));
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
    clock: Res<Clock>,
) {
    if clock.0 % 6 != 0 {
        return;
    }

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

pub fn build_flow_fields(
    mut commands: Commands,
    query: Query<(&TeamAlignment, &Position)>,
    terrain: Res<TerrainMap>,
    clock: Res<Clock>,
) {
    if (clock.0 % 6) != 0 {
        return;
    }
    let mut alignment_to_cells = HashMap::<TeamValue, HashSet<SpatialHashCell>>::new();

    // Build sets of cells that contain units of each alignment
    for (alignment, position) in query.iter() {
        if !alignment_to_cells.contains_key(&alignment.alignment) {
            alignment_to_cells.insert(alignment.alignment, HashSet::new());
        }
        if f32::is_nan(position.pos.x) || f32::is_nan(position.pos.y) {
            continue;
        }
        if let Some(set) = alignment_to_cells.get_mut(&alignment.alignment) {
            let spatial_hash = get_point_spatial_hash(position.pos, terrain.cell_size);
            set.insert(spatial_hash);
        }
    }

    let mut integration_field = HashMap::<SpatialHashCell, HashMap<TeamValue, f32>>::new();
    let mut flow_field = HashMap::<SpatialHashCell, HashMap<TeamValue, Vector2>>::new();

    // Build integration field for each alignment
    // Integration part (1)
    for alignment in alignment_to_cells.keys() {
        let mut open_set: HashSet<SpatialHashCell> = HashSet::new();
        // Build open set as cells of all enemy units
        for (enemy_alignment, cells) in alignment_to_cells.iter() {
            if *alignment == *enemy_alignment {
                continue;
            }
            for cell in cells {
                if *cell == SpatialHashCell(0, 0) {}
                open_set.insert(*cell);
            }
        }

        let team = *alignment;

        // Convert set to queue and zero out goal nodes
        let mut open_queue: VecDeque<SpatialHashCell> = VecDeque::new();
        for cell in open_set.drain() {
            open_queue.push_back(cell);
            set_spatial_team_value(&mut integration_field, cell, team, 0.0);
        }

        // Integration part (2)
        // Compute integration field
        while !open_queue.is_empty() {
            let node = open_queue.pop_front().unwrap();
            let curr_node_val = get_spatial_team_value(&integration_field, node, team, f32::MAX);
            let mut orthognal_obstacle_x: Vec<i32> = Vec::new();
            let mut orthognal_obstacle_y: Vec<i32> = Vec::new();
            // First check orthognal directions. Only allow diagonals without an obstacle in the corresponding orthognal directions.
            for neighbor in get_orthognal_neighbor_cells(node, terrain.max_bounds) {
                let terrain_cell = terrain.get_cell(neighbor);
                if terrain_cell.pathable_mask == 0 {
                    set_spatial_team_value(&mut integration_field, neighbor, team, f32::MAX);
                    orthognal_obstacle_x.push(neighbor.0);
                    orthognal_obstacle_y.push(neighbor.1);
                    continue;
                }

                let cost = terrain_cell.movement_cost;
                let potential_path_cost = curr_node_val + cost;
                let neighbor_val =
                    get_spatial_team_value(&integration_field, neighbor, team, f32::MAX);
                if potential_path_cost < neighbor_val {
                    open_queue.push_back(neighbor);
                    set_spatial_team_value(
                        &mut integration_field,
                        neighbor,
                        team,
                        potential_path_cost,
                    );
                }
            }

            for neighbor in get_diagonal_neighbor_cells(node, terrain.max_bounds) {
                let terrain_cell = terrain.get_cell(neighbor);
                if terrain_cell.pathable_mask == 0 {
                    set_spatial_team_value(&mut integration_field, neighbor, team, f32::MAX);
                    continue;
                }
                if orthognal_obstacle_x.contains(&neighbor.0)
                    || orthognal_obstacle_y.contains(&neighbor.1)
                {
                    continue;
                }
                let cost = terrain_cell.movement_cost;
                let potential_path_cost = curr_node_val + cost;
                let neighbor_val =
                    get_spatial_team_value(&integration_field, neighbor, team, f32::MAX);
                if potential_path_cost < neighbor_val {
                    open_queue.push_back(neighbor);
                    set_spatial_team_value(
                        &mut integration_field,
                        neighbor,
                        team,
                        potential_path_cost,
                    );
                }
            }
        }

        // Build flow_field from integration field
        let max_convolution_length: i32 = 1;
        for cell in integration_field.keys() {
            let mut flow = Vector2::ZERO;
            let integration_val_of_cell =
                get_spatial_team_value(&integration_field, *cell, team, f32::MAX);
            if integration_val_of_cell <= 0.01 {
                continue;
            }

            // SPECIAL CASE: Convolution length 1
            // Constraints:
            // --If an obstacle is encountered, pick the min integration value rather than convoluted value (convoluted = double entendre :)
            // --Disallow diagonal movement through obstacles
            let mut orthognal_obstacle_x: Vec<i32> = Vec::new();
            let mut orthognal_obstacle_y: Vec<i32> = Vec::new();
            let mut min_node: SpatialHashCell = SpatialHashCell(0, 0);
            let mut min_integration_val = f32::MAX;
            for neighbor in get_orthognal_neighbor_cells(*cell, terrain.max_bounds) {
                let terrain_cell = terrain.get_cell(neighbor);
                if terrain_cell.pathable_mask == 0 {
                    orthognal_obstacle_x.push(neighbor.0);
                    orthognal_obstacle_y.push(neighbor.1);
                    continue;
                }
                let integration_val =
                    get_spatial_team_value(&integration_field, neighbor, team, f32::MAX).max(0.01);
                if integration_val < min_integration_val {
                    min_integration_val = integration_val;
                    min_node = neighbor
                }

                // Treating this as convolution length 1 for the case where no obstacles are found
                flow += normalized_or_zero(Vector2 {
                    x: (neighbor.0 - cell.0) as f32,
                    y: (neighbor.1 - cell.1) as f32,
                }) * (1000. / integration_val);
            }

            for neighbor in get_diagonal_neighbor_cells(*cell, terrain.max_bounds) {
                // Don't allow diagonal flow through obstacles
                if orthognal_obstacle_x.contains(&neighbor.0)
                    || orthognal_obstacle_y.contains(&neighbor.1)
                {
                    continue;
                }

                let terrain_cell = terrain.get_cell(neighbor);
                if terrain_cell.pathable_mask == 0 {
                    // Values will never be relevant; just to mark that an obstacle was found at grid distance 1
                    orthognal_obstacle_x.push(-1);
                    orthognal_obstacle_y.push(-1);
                    continue;
                }

                let integration_val =
                    get_spatial_team_value(&integration_field, neighbor, team, f32::MAX).max(0.01);
                if integration_val < min_integration_val {
                    min_integration_val = integration_val;
                    min_node = neighbor
                }
                // Treating this as convolution length 1 for the case where no obstacles are found
                flow += normalized_or_zero(Vector2 {
                    x: (neighbor.0 - cell.0) as f32,
                    y: (neighbor.1 - cell.1) as f32,
                }) * (1000. / integration_val);
            }

            // Only go into convolution loop if no obstacle was found at distance 1
            if orthognal_obstacle_x.is_empty() {
                let mut obstacle_encountered: bool = false;
                let mut convolution_length = 2;
                loop {
                    // (1) Each iteration, we get the convolution flow vectors and
                    // add them all to the output flow
                    // (2) UNLESS we've found an obstacle at this convolution range,
                    // Then we use the previous distance's summed flow field.
                    let mut temp_flow = Vector2::ZERO;

                    for neighbor in get_convolution_neighbor_cells_increment(
                        *cell,
                        convolution_length,
                        terrain.max_bounds,
                    ) {
                        let integration_val =
                            get_spatial_team_value(&integration_field, neighbor, team, f32::MAX)
                                .max(0.01);
                        // (2) Detection
                        if terrain.get_cell(neighbor).pathable_mask == 0 {
                            obstacle_encountered = true;
                            continue;
                        }

                        // (1) Base case
                        temp_flow += normalized_or_zero(Vector2 {
                            x: (neighbor.0 - cell.0) as f32,
                            y: (neighbor.1 - cell.1) as f32,
                        }) * (1000. / integration_val);
                    }
                    if obstacle_encountered {
                        // (2) Exit out, using previous length convolution flow
                        break;
                    } else {
                        // (1) Base case
                        flow += temp_flow;
                    }
                    // (1) If no obstacles encountered, finish when max length reached
                    convolution_length += 1;
                    if convolution_length > max_convolution_length {
                        break;
                    }
                }
            } else {
                // Fallback -- obstacle at distance 1
                // Override calculated flow sum with minimum next-door neighbor integration value
                flow = normalized_or_zero(Vector2 {
                    x: (min_node.0 - cell.0) as f32,
                    y: (min_node.1 - cell.1) as f32,
                });
            }
            set_spatial_team_value(&mut flow_field, *cell, team, normalized_or_zero(flow));
        }
    }

    // Insert resulting resource
    commands.insert_resource(integration_field);
    commands.insert_resource(FlowFieldsTowardsEnemies {
        map: flow_field,
        cell_size: terrain.cell_size,
    });
}
