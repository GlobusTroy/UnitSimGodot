use gdnative::prelude::*;

pub fn true_distance(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_to(pos2) - (rad1 + rad2)
}

pub fn true_distance_squared(pos1: Vector2, pos2: Vector2, rad1: f32, rad2: f32) -> f32 {
    pos1.distance_squared_to(pos2) - ((rad1 + rad2) * (rad1 + rad2))
}

pub fn get_point_spatial_hash(point: Vector2, cell_size: f32) -> (i32, i32) {
    ((point.x / cell_size) as i32, (point.y / cell_size) as i32)
}