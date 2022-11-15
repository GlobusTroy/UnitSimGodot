use std::collections::HashMap;

use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    graphics::CleanupCanvasItem,
    physics::{spatial_structures::SpatialHashCell, DeltaPhysics},
    unit::TeamValue,
};

#[derive(Component)]
pub struct ExpirationTimer(pub f32);

pub fn expire_entities(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut ExpirationTimer,
        Option<&crate::graphics::Renderable>,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut timer, render_option) in query.iter_mut() {
        if timer.0 < 0.0 {
            commands.entity(entity).remove::<ExpirationTimer>();
            continue;
        }
        timer.0 -= delta.seconds;
        if timer.0 < 0.0 {
            commands.entity(entity).despawn();
            if let Some(renderable) = render_option {
                commands
                    .spawn()
                    .insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
        }
    }
}

pub fn true_distance(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_to(pos2) - (rad1 + rad2)
}

pub fn true_distance_squared(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_squared_to(pos2) - ((rad1 + rad2) * (rad1 + rad2))
}

pub fn get_point_spatial_hash(point: Vector2, cell_size: f32) -> SpatialHashCell {
    SpatialHashCell((point.x / cell_size) as i32, (point.y / cell_size) as i32)
}

pub fn get_octognal_neighbor_cells(
    cell: SpatialHashCell,
    bounds: SpatialHashCell,
) -> Vec<(SpatialHashCell, f32)> {
    let mut out: Vec<(SpatialHashCell, f32)> = Vec::new();
    for x in cell.0 - 1..cell.0 + 2 {
        for y in cell.1 - 1..cell.1 + 2 {
            if x == cell.0 && y == cell.1 {
                continue;
            }
            if x >= -1 && y >= -1 && x <= bounds.0 && y <= bounds.1 {
                let dist = x.abs_diff(cell.0) + y.abs_diff(cell.1);
                out.push((SpatialHashCell(x, y), dist as f32));
            }
        }
    }
    return out;
}

pub fn get_diagonal_neighbor_cells(
    cell: SpatialHashCell,
    bounds: SpatialHashCell,
) -> Vec<SpatialHashCell> {
    let mut out: Vec<SpatialHashCell> = Vec::new();
    for x in cell.0 - 1..cell.0 + 2 {
        for y in cell.1 - 1..cell.1 + 2 {
            if x == cell.0 || y == cell.1 {
                continue;
            }
            if x >= -1 && y >= -1 && x <= bounds.0 && y <= bounds.1 {
                out.push(SpatialHashCell(x, y));
            }
        }
    }
    return out;
}

pub fn get_orthognal_neighbor_cells(
    cell: SpatialHashCell,
    bounds: SpatialHashCell,
) -> Vec<SpatialHashCell> {
    let mut out: Vec<SpatialHashCell> = Vec::new();
    for x in cell.0 - 1..cell.0 + 2 {
        if x >= -1 && x <= bounds.0 && x != cell.0 {
            out.push(SpatialHashCell(x, cell.1));
        }
    }
    for y in cell.1 - 1..cell.1 + 2 {
        if y >= -1 && y <= bounds.1 && y != cell.1 {
            out.push(SpatialHashCell(cell.0, y));
        }
    }
    return out;
}

pub fn get_convolution_neighbor_cells(
    cell: SpatialHashCell,
    convolution_size: i32,
    bounds: SpatialHashCell,
) -> Vec<SpatialHashCell> {
    let mut out: Vec<SpatialHashCell> = Vec::new();
    for x in cell.0 - convolution_size..cell.0 + convolution_size + 1 {
        for y in cell.1 - convolution_size..cell.1 + convolution_size + 1 {
            if x >= -1 && y >= -1 && x <= bounds.0 && y <= bounds.1 {
                if x == cell.0 && y == cell.1 {
                    continue;
                }
                out.push(SpatialHashCell(x, y));
            }
        }
    }
    return out;
}

pub fn get_convolution_neighbor_cells_increment(
    cell: SpatialHashCell,
    convolution_size: i32,
    bounds: SpatialHashCell,
) -> Vec<SpatialHashCell> {
    let mut out: Vec<SpatialHashCell> = Vec::new();
    for x in cell.0 - convolution_size..cell.0 + convolution_size + 1 {
        let y1 = cell.1 - convolution_size;
        let y2 = cell.1 + convolution_size;
        if x >= 0 && y1 >= 0 && x <= bounds.0 && y2 <= bounds.1 {
            out.push(SpatialHashCell(x, y1));
            out.push(SpatialHashCell(x, y2));
        }
    }
    for y in cell.1 - convolution_size..cell.1 + convolution_size + 1 {
        let x1 = cell.0 - convolution_size;
        let x2 = cell.0 + convolution_size;
        if x1 >= 0 && y >= 0 && x2 <= bounds.0 && y <= bounds.1 {
            out.push(SpatialHashCell(x1, y));
            out.push(SpatialHashCell(x2, y));
        }
    }
    return out;
}

pub fn set_spatial_team_value<T>(
    spatial_team_field: &mut HashMap<SpatialHashCell, HashMap<TeamValue, T>>,
    cell: SpatialHashCell,
    team: TeamValue,
    value: T,
) {
    if let Some(team_to_integration) = spatial_team_field.get_mut(&cell) {
        team_to_integration.insert(team, value);
    } else {
        let mut integration_cell: HashMap<TeamValue, T> = HashMap::new();
        integration_cell.insert(team, value);
        spatial_team_field.insert(cell, integration_cell);
    }
}

pub fn get_spatial_team_value<T: Copy>(
    spatial_team_field: &HashMap<SpatialHashCell, HashMap<TeamValue, T>>,
    cell: SpatialHashCell,
    team: TeamValue,
    default: T,
) -> T {
    if let Some(team_to_integration) = spatial_team_field.get(&cell) {
        return match team_to_integration.get(&team) {
            Some(val) => *val,
            None => default,
        };
    } else {
        return default;
    }
}

pub fn normalized_or_zero(v: Vector2) -> Vector2 {
    if v.length() <= f32::EPSILON {
        Vector2::ZERO
    } else {
        v.normalized()
    }
}
