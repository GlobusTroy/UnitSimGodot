use crate::{
    graphics::animation::PlayAnimationDirective,
    physics::{self, spatial_structures::*, *},
    unit::{ProjectileWeapon, Channeling, TeamAlignment, TeamValue, Stunned},
    util::{get_point_spatial_hash, get_spatial_team_value, normalized_or_zero, true_distance},
};
use bevy_ecs::prelude::*;
use gdnative::prelude::*;

pub mod conductors;

#[derive(Bundle)]
struct StandardBoids {
    avoid_walls: AvoidWallsBoid,
    separation_boid: SeparationBoid,
    seek_enemies_boid: SeekEnemiesBoid,
    stopping_boid: StoppingBoid,
    cohesion_boid: CohesionBoid,
    alignment_boid: VectorAlignmentBoid,
}

#[derive(Component)]
pub struct AvoidWallsBoid {
    pub avoidance_radius: f32,
    pub cell_size_multiplier: f32,
    pub multiplier: f32,
}

#[derive(Component)]
pub struct SeparationBoid {
    pub avoidance_radius: f32,
    pub multiplier: f32,
}

#[derive(Component, Clone, Copy)]
pub struct SeekEnemiesBoid {
    pub multiplier: f32,
}

#[derive(Component)]
pub struct StoppingBoid {
    pub multiplier: f32,
}

#[derive(Component)]
pub struct CohesionBoid {
    pub cohesion_radius: f32,
    pub multiplier: f32,
}

#[derive(Component)]
pub struct VectorAlignmentBoid {
    pub alignment_radius: f32,
    pub multiplier: f32,
}

#[derive(Component, Clone, Copy)]
pub struct ChargeAtEnemyBoid {
    pub charge_radius: f32,
    pub multiplier: f32,
    pub target: Option<Entity>,
    pub target_timer: f32,
}

#[derive(Component, Clone, Copy)]
pub struct KiteNearestEnemyBoid {
    pub kite_radius: f32,
    pub multiplier: f32,
}

#[derive(Component)]
pub struct AppliedBoidForces(pub Vector2);

#[derive(Component)]
pub struct BoidParams {
    pub max_speed: f32,
    pub max_force: f32,
}

pub fn boid_apply_params(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut AppliedForces,
        &mut AppliedBoidForces,
        &BoidParams,
        Option<&crate::graphics::animation::AnimatedSprite>,
        Option<&mut Velocity>,
        Option<&PlayAnimationDirective>,
    )>,
) {
    for (
        entity,
        mut forces,
        mut boid_forces,
        params,
        sprite_option,
        velocity_option,
        animation_directive_option,
    ) in query.iter_mut()
    {
        if boid_forces.0.length() > params.max_force {
            boid_forces.0 = boid_forces.0.normalized() * params.max_force;
        }

        if let Some(sprite) = sprite_option {
            if let Some(mut velocity) = velocity_option {
                // Clamp velocity
                velocity.v = velocity.v.clamped(params.max_speed);

                // Idle animation / run animation cues
                if let None = animation_directive_option {
                    if sprite.animation_name == "idle".to_string()
                        && velocity.v.length() >= (params.max_speed / 4.)
                    {
                        commands.entity(entity).insert(PlayAnimationDirective {
                            animation_name: "run".to_string(),
                            is_one_shot: false,
                        });
                    } else if sprite.animation_name == "run".to_string()
                        && velocity.v.length() <= params.max_speed / 4.
                    {
                        commands.entity(entity).insert(PlayAnimationDirective {
                            animation_name: "idle".to_string(),
                            is_one_shot: false,
                        });
                    }
                }
            }
        }

        forces.0 += boid_forces.0;
        boid_forces.0 = Vector2::ZERO;
    }
}

pub fn kite_enemies_boid(
    mut query: Query<(
        Entity,
        &mut AppliedBoidForces,
        &BoidParams,
        &KiteNearestEnemyBoid,
        &TeamAlignment,
        &Position,
        &Radius,
        &Velocity,
    )>,
    target_query: Query<(&TeamAlignment, &Position, &Radius)>,
    spatial: Res<SpatialNeighborsCache>,
) {
    for (entity, mut forces, params, boid, alignment, position, radius, velocity) in
        query.iter_mut()
    {
        if let Some(neighbors) = spatial.get_neighbors(&entity, boid.kite_radius) {
            let mut min_distance = boid.kite_radius;
            let mut min_pos = position.pos;

            for neighbor in neighbors {
                if let Ok((alignment2, position2, radius2)) = target_query.get(neighbor) {
                    if alignment2.alignment != TeamValue::NeutralPassive
                        && alignment.alignment != alignment2.alignment
                    {
                        let distance =
                            true_distance(position.pos, position2.pos, radius.r, radius2.r);
                        if distance < min_distance {
                            min_distance = distance;
                            min_pos = position2.pos;
                        }
                    }
                }
            }
            if min_pos == position.pos {
                continue;
            }

            let desired_velocity = normalized_or_zero(position.pos - min_pos) * params.max_speed;
            let force = normalized_or_zero(desired_velocity - velocity.v)
                * params.max_force
                * boid.multiplier;
            forces.0 += force;
        }
    }
}

pub fn charge_at_enemy_boid(
    mut query: Query<
        (
            Entity,
            &mut AppliedBoidForces,
            &BoidParams,
            &mut ChargeAtEnemyBoid,
            &TeamAlignment,
            &Position,
            &Radius,
            &Velocity,
        ),
        (Without<Channeling>, Without<Stunned>),
    >,
    target_query: Query<(Entity, &TeamAlignment, &Position, &Radius)>,
    spatial: Res<SpatialNeighborsCache>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut forces, params, mut boid, alignment, position, radius, velocity) in
        query.iter_mut()
    {
        boid.target_timer += delta.seconds;
        if boid.target_timer >= 0.1 {
            boid.target_timer = 0.0;
            if let Some(neighbors) = spatial.get_neighbors(&entity, boid.charge_radius) {
                let mut min_distance = f32::MAX;
                for neighbor in neighbors {
                    if let Ok((ent_target, alignment2, position2, radius2)) = target_query.get(neighbor) {
                        if alignment2.alignment != TeamValue::NeutralPassive
                            && alignment.alignment != alignment2.alignment
                        {
                            let distance =
                                true_distance(position.pos, position2.pos, radius.r, radius2.r);
                            if distance < min_distance {
                                min_distance = distance;
                                boid.target = Some(ent_target);
                            }
                        }
                    }
                }
            }
        }  

        if let Some(target) = boid.target {
            if let Ok((_,_,position_target,_)) = target_query.get(target) {
                let desired_velocity = normalized_or_zero(position_target.pos - position.pos) * params.max_speed;
                let force = normalized_or_zero(desired_velocity - velocity.v) * params.max_force;
                forces.0 += force * boid.multiplier;
            }
        }
    }
}

pub fn seek_enemies_boid(
    mut query: Query<
        (
            &mut AppliedBoidForces,
            &BoidParams,
            &SeekEnemiesBoid,
            &TeamAlignment,
            &Position,
            &Velocity,
        ),
        (Without<Channeling>, Without<Stunned>)
    >,
    flow_field: Res<FlowFieldsTowardsEnemies>,
) {
    for (mut forces, params, boid, alignment, position, velocity) in query.iter_mut() {
        let team = alignment.alignment;
        let cell = get_point_spatial_hash(position.pos, flow_field.cell_size);
        let desired_velocity = normalized_or_zero(get_spatial_team_value(
            &flow_field.map,
            cell,
            team,
            Vector2::ZERO,
        )) * params.max_speed;
        if desired_velocity == Vector2::ZERO {
            continue;
        }
        let flow_force = normalized_or_zero(desired_velocity - velocity.v) * params.max_force;
        forces.0 += flow_force * boid.multiplier;
    }
}

pub fn vector_alignment_boid(
    mut query: Query<
        (
            Entity,
            &mut AppliedBoidForces,
            &BoidParams,
            &VectorAlignmentBoid,
            &TeamAlignment,
            &Velocity,
        ),
        (Without<Channeling>, Without<Stunned>),
    >,
    inner_query: Query<(&TeamAlignment, &Velocity, &Mass)>,
    spatial: Res<SpatialNeighborsCache>,
) {
    for (entity, mut forces, params, boid, alignment, velocity) in query.iter_mut() {
        let mut momentum_sum = Vector2::ZERO;
        let neighbor_group = spatial.get_neighbors(&entity, boid.alignment_radius);
        if let Some(entities) = neighbor_group {
            for entity_test in entities.iter() {
                if let Ok((alignment_test, velocity_test, mass_test)) =
                    inner_query.get(*entity_test)
                {
                    if alignment.alignment != alignment_test.alignment {
                        continue;
                    }
                    momentum_sum += velocity_test.v * mass_test.0;
                }
            }
        }
        let desired_velocity = normalized_or_zero(momentum_sum) * params.max_speed;
        let flocking_force =
            (desired_velocity - velocity.v).clamped(params.max_force) * boid.multiplier;
        forces.0 += flocking_force;
    }
}

pub fn cohesion_boid(
    mut query: Query<
        (
            Entity,
            &mut AppliedBoidForces,
            &BoidParams,
            &CohesionBoid,
            &TeamAlignment,
            &Position,
            &Velocity,
        ),
        (Without<Channeling>, Without<Stunned>)
    >,
    inner_query: Query<(&TeamAlignment, &Position, &Mass)>,
    spatial: Res<SpatialNeighborsCache>,
) {
    for (entity, mut forces, params, boid, alignment, position, velocity) in query.iter_mut() {
        let mut mass_neighbors = 0.;
        let mut center_of_mass_sum = Vector2::ZERO;
        let neighbor_group = spatial.get_neighbors(&entity, boid.cohesion_radius);
        if let Some(entities) = neighbor_group {
            for entity_test in entities.iter() {
                if let Ok((alignment_test, position_test, mass_test)) =
                    inner_query.get(*entity_test)
                {
                    if alignment.alignment != alignment_test.alignment {
                        continue;
                    }
                    center_of_mass_sum += position_test.pos * mass_test.0;
                    mass_neighbors += mass_test.0;
                }
            }
        }
        if mass_neighbors < 0.01 {
            continue;
        }
        let flock_center_of_mass = center_of_mass_sum / mass_neighbors;
        let desired_velocity =
            normalized_or_zero(flock_center_of_mass - position.pos) * params.max_speed;
        let flocking_force =
            (desired_velocity - velocity.v).clamped(params.max_force) * boid.multiplier;
        forces.0 += flocking_force;
    }
}

pub fn stopping_boid(
    mut query: Query<
        (
            &mut AppliedBoidForces,
            &BoidParams,
            &StoppingBoid,
            &Velocity,
        ),
        Or<(With<Channeling>, With<Stunned>)>,
    >,
    delta: Res<DeltaPhysics>,
) {
    for (mut forces, params, boid, velocity) in query.iter_mut() {
        forces.0 -= normalized_or_zero(velocity.v)
            * params.max_force.min(velocity.v.length() / delta.seconds)
            * boid.multiplier;
    }
}

pub fn avoid_walls_boid(
    mut query: Query<(
        &mut AppliedBoidForces,
        &BoidParams,
        &AvoidWallsBoid,
        &Position,
        &Velocity,
        &Radius,
    )>,
    map: Res<TerrainMap>,
) {
    for (mut forces, params, boid, position, velocity, radius) in query.iter_mut() {
        let mut nearest_wall_dist = f32::MAX;
        let mut force_change: Vector2 = Vector2::ZERO;
        for spatial_hash in get_all_spatial_hashes_from_circle(
            position.pos,
            radius.r + map.cell_size,
            map.cell_size,
        )
        .iter()
        {
            if map.get_cell(*spatial_hash).pathable_mask == 0 {
                let terrain_pos = Vector2 {
                    x: map.cell_size * spatial_hash.0 as f32 + map.cell_size / 2.,
                    y: map.cell_size * spatial_hash.1 as f32 + map.cell_size / 2.,
                };
                let distance = crate::util::true_distance(
                    position.pos,
                    terrain_pos,
                    radius.r,
                    map.cell_size * boid.cell_size_multiplier,
                );
                if distance >= nearest_wall_dist {
                    continue;
                }

                nearest_wall_dist = distance;

                let desired_vel = terrain_pos.direction_to(position.pos) * params.max_speed;
                let avoidance_force = (desired_vel - velocity.v).clamped(params.max_force);
                let distance_multiplier = boid.avoidance_radius / distance.max(0.001);
                force_change = avoidance_force * distance_multiplier * boid.multiplier;
            }
        }
        forces.0 += force_change;
    }
}

pub fn separation_boid(
    mut query: Query<
        (
            Entity,
            &mut AppliedBoidForces,
            &BoidParams,
            &SeparationBoid,
            &TeamAlignment,
            &Position,
            &Velocity,
            &Radius,
        ),
        (Without<Channeling>, Without<Stunned>)
    >,
    inner_query: Query<(Entity, &TeamAlignment, &Position, &Radius), With<Mass>>,
    spatial: Res<physics::spatial_structures::SpatialNeighborsCache>,
) {
    for (entity, mut forces, params, boid, alignment, position, velocity, radius) in
        query.iter_mut()
    {
        let neighbor_group = spatial.get_neighbors(&entity, boid.avoidance_radius);
        if let Some(entities) = neighbor_group {
            for entity_test in entities.iter() {
                if let Ok((_, alignment_test, position_test, radius_test)) =
                    inner_query.get(*entity_test)
                {
                    if alignment.alignment != alignment_test.alignment {
                        continue;
                    }
                    let distance = crate::util::true_distance(
                        position.pos,
                        position_test.pos,
                        radius_test.r,
                        radius.r,
                    );
                    if distance < boid.avoidance_radius {
                        let desired_vel =
                            position_test.pos.direction_to(position.pos) * params.max_speed;
                        let separation_force = (desired_vel - velocity.v).clamped(params.max_force);
                        let distance_multiplier = boid.avoidance_radius / distance.max(0.001);
                        forces.0 += separation_force * distance_multiplier * boid.multiplier;
                    }
                }
            }
        }
    }
}
