use crate::physics::{self, *};
use bevy_ecs::prelude::*;
use gdnative::prelude::*;

#[derive(Component)]
pub struct SeparationBoid {
    pub avoidance_radius: f32,
    pub multiplier: f32,
}

#[derive(Component)]
pub struct StoppingBoid {
    pub multiplier: f32,
}

#[derive(Component)]
pub struct FlockingBoid {
    pub flocking_radius: f32,
    pub flocking_multiplier: f32,
}

#[derive(Component)]
pub struct BoidParams {
    pub max_speed: f32,
    pub max_force: f32,
}

pub fn boid_apply_params(mut query: Query<(&mut AppliedForces, &BoidParams)>) {
    for (mut forces, params) in query.iter_mut() {
        if forces.0.length() > params.max_force {
            forces.0 = forces.0.normalized() * params.max_force;
        }
    }
}

// pub fn seeking_boid(mut query: Query<(&mut AppliedForces, &BoidParams, &SeekingBoid, &Position, &Velocity)>) {
//     for (mut forces, params, boid, position, velocity) in query.iter_mut() {

//     }
// }

pub fn stopping_boid(
    mut query: Query<(&mut AppliedForces, &BoidParams, &StoppingBoid, &Velocity)>,
) {
    for (mut forces, params, boid, velocity) in query.iter_mut() {
        forces.0 += -velocity.v * params.max_force.min(velocity.v.length()) * boid.multiplier;
    }
}

pub fn separation_boid(
    mut query: Query<(
        Entity,
        &mut AppliedForces,
        &BoidParams,
        &SeparationBoid,
        &Position,
        &Velocity,
        &Radius,
    )>,
    inner_query: Query<(Entity, &Position, &Radius), With<Mass>>,
    spatial: Res<physics::spatial_structures::SpatialNeighborsCache>,
) {
    for (entity, mut forces, params, boid, position, velocity, radius) in query.iter_mut() {
        let neighbor_group = spatial.get_neighbors(&entity, boid.avoidance_radius);
        if let Some(entities) = neighbor_group {
            for entity_test in entities.iter() {
                if let Ok((_, position_test, radius_test)) = inner_query.get(*entity_test) {
                    let distance = crate::util::true_distance(
                        position.pos,
                        position_test.pos,
                        radius_test.r,
                        radius.r,
                    );
                    if distance < boid.avoidance_radius {
                        let desired_vel =
                            position_test.pos.direction_to(position.pos) * params.max_speed;
                        let separation_force =
                            (desired_vel - velocity.v).normalized() * params.max_force;
                        let distance_multiplier = boid.avoidance_radius / distance.max(0.1);
                        forces.0 += separation_force * distance_multiplier * boid.multiplier;
                    }
                }
            }
        }
    }
}
