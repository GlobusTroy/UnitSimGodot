use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    graphics::CleanupCanvasItem,
    physics::{spatial_structures::SpatialHashTable, DeltaPhysics, Position, Radius, Velocity},
    util::{normalized_or_zero, true_distance, ExpirationTimer},
};

use super::{
    abilities::SlowPoisonAttack, Channeling, DamageInstance, Hitpoints, SlowPoisonDebuff,
    StunOnHitEffect, Stunned, TeamAlignment,
};

#[derive(Component)]
pub struct UnitActions {
    vec: Vec<Entity>,
}

// Reusable action attached to a unit
#[derive(Component)]
pub struct ActionEntity {
    pub owner: Entity,
}

#[derive(Component)]
pub struct DeathApproaches {
    pub spawn_corpse: bool,
    pub cleanup_corpse_canvas: bool,
    pub cleanup_time: f32,
}

// Instance of an action occurring, attached to an ActionEntity
#[derive(Component)]
pub struct ActionInstanceEntity {
    pub owner: Entity,
}

#[derive(Component)]
pub struct ActionEffectApplied(bool);

#[derive(Component)]
pub struct SwingDetails {
    pub impact_time: f32,
    pub complete_time: f32,
    pub cooldown_time: f32,
}

#[derive(Component)]
pub struct ActionProjectileDetails {
    pub projectile_speed: f32,
    pub projectile_scale: f32,
    pub projectile_texture: Rid,
    pub contact_distance: f32,
}

#[derive(Component)]
pub struct Cooldown(pub f32);

#[derive(Component)]
pub struct Caster {
    pub entity: Entity,
}

#[derive(Component)]
pub struct TargetEntity {
    pub entity: Entity,
    pub other_targets: Vec<Entity>,
}

#[derive(Component)]
pub struct TargetPosition {
    pub pos: Position,
}

#[derive(Component)]
pub struct ChannelingDetails {
    pub total_time_channeled: f32,
}

pub enum ImpactType {
    Instant,
    Projectile,
}

#[derive(Component)]
pub struct ActionImpactType(pub ImpactType);

#[derive(Component)]
pub struct TargetsEnemies {}

#[derive(Component)]
pub struct TargetsAllies {}

#[derive(Component)]
pub struct TargetFlags {
    ignore_full_health: bool,
    ignore_no_debuff: bool,
    ignore_no_buff: bool,
    target_enemies: bool,
    target_allies: bool,
}

#[derive(Component)]
pub struct ActionRange(pub f32);

#[derive(Component)]
pub struct Splash {
    pub radius: f32,
    pub effect_ratio: f32,
}

#[derive(Component)]
pub struct Cleave {
    pub angle_degrees: f32,
    pub effect_ratio: f32,
}

#[derive(Component)]
pub struct OnHitEffects {
    vec: Vec<Effect>,
}

#[derive(Component)]
pub struct Projectile {
    pub target: Entity,
    pub target_pos: Vector2,
    pub origin_action: Entity,
}

#[derive(Copy, Clone)]
pub enum Effect {
    DamageEffect(DamageInstance),
    PoisonEffect(SlowPoisonAttack),
    StunEffect(StunOnHitEffect),
}

#[derive(Component)]
pub struct ResolveEffectsBuffer {
    pub vec: Vec<Effect>,
}

#[derive(Component)]
pub struct PerformingActionState {
    pub action: Entity,
}

pub fn action_cooldown(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Cooldown)>,
    delta: Res<DeltaPhysics>,
) {
    for (ent, mut cooldown) in query.iter_mut() {
        cooldown.0 -= delta.seconds;
        if cooldown.0 <= 0.0 {
            commands.entity(ent).remove::<Cooldown>();
        }
    }
}

fn spawn_projectile(
    commands: &mut Commands,
    origin_action: Entity,
    origin_pos: Vector2,
    target: Entity,
    details: &ActionProjectileDetails,
) {
    commands
        .spawn()
        .insert(Projectile {
            target: target,
            target_pos: Vector2::ZERO,
            origin_action: origin_action,
        })
        .insert(Position { pos: origin_pos })
        .insert(Velocity { v: Vector2::ZERO })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(crate::graphics::animation::AnimatedSprite {
            // Will be overriden by play animation directive
            texture: details.projectile_texture,
            animation_name: "fly".to_string(),
            animation_index: 0,
            animation_speed: 1.,
            animation_time_since_change: 0.,
            animation_length: 100,
            is_one_shot: false,
        })
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .insert(crate::graphics::ScaleSprite(Vector2 {
            x: details.projectile_scale,
            y: details.projectile_scale,
        }));
}

pub fn performing_action_state(
    mut commands: Commands,
    query: Query<(Entity, &PerformingActionState, &Position, &Radius)>,
    mut action_query: Query<(
        Entity,
        &mut ChannelingDetails,
        &SwingDetails,
        &mut ActionEffectApplied,
        &ActionImpactType,
        &TargetEntity,
        &OnHitEffects,
        Option<&ActionProjectileDetails>,
        Option<&Cleave>,
        Option<&ActionRange>,
    )>,
    mut apply_query: Query<&mut ResolveEffectsBuffer>,
    pos_query: Query<(&Position, &Radius)>,
    spatial: Res<SpatialHashTable>,
    delta: Res<DeltaPhysics>,
) {
    for (ent, performer, position, radius) in query.iter() {
        if let Ok((
            ent_action,
            mut channeling,
            swing,
            mut already_applied,
            impact_type,
            target,
            effects,
            projectile_option,
            cleave_option,
            range_option,
        )) = action_query.get_mut(performer.action)
        {
            channeling.total_time_channeled += delta.seconds;

            if channeling.total_time_channeled >= swing.impact_time && !already_applied.0 {
                match impact_type.0 {
                    ImpactType::Instant => {
                        if let Ok(mut buffer) = apply_query.get_mut(target.entity) {
                            for effect in effects.vec.iter() {
                                buffer.vec.push(*effect);
                            }
                        }

                        // Handle cleave
                        if let Some(cleave) = cleave_option {
                            if let Some(range) = range_option {
                                for cell in crate::get_all_spatial_hashes_from_circle(
                                    position.pos,
                                    range.0,
                                    spatial.cell_size,
                                ) {
                                    if let Some(potential_targets) = spatial.table.get(&cell) {
                                        let mut already_effected = std::collections::HashSet::new();
                                        already_effected.insert(target.entity);
                                        if let Ok((original_target_pos, _)) =
                                            pos_query.get(target.entity)
                                        {
                                            let to_target = original_target_pos.pos - position.pos;
                                            for potential_splash_target in potential_targets.iter()
                                            {
                                                if already_effected
                                                    .contains(potential_splash_target)
                                                {
                                                    continue;
                                                }
                                                if let Ok((splash_target_pos, splash_target_rad)) =
                                                    pos_query.get(*potential_splash_target)
                                                {
                                                    if true_distance(
                                                        position.pos,
                                                        splash_target_pos.pos,
                                                        radius.r,
                                                        splash_target_rad.r,
                                                    ) <= range.0
                                                    {
                                                        already_effected
                                                            .insert(*potential_splash_target);
                                                        // Check for angle
                                                        let to_cleave =
                                                            splash_target_pos.pos - position.pos;
                                                        if to_target
                                                            .angle_to(to_cleave)
                                                            .to_degrees()
                                                            .abs()
                                                            <= cleave.angle_degrees
                                                        {
                                                            // Apply effects to cleave targets
                                                            if let Ok(mut buffer) = apply_query
                                                                .get_mut(*potential_splash_target)
                                                            {
                                                                for effect in effects.vec.iter() {
                                                                    buffer.vec.push(*effect);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ImpactType::Projectile => {
                        if let Some(details) = projectile_option {
                            spawn_projectile(
                                &mut commands,
                                ent_action,
                                position.pos,
                                target.entity,
                                details,
                            );
                        }
                    }
                    _ => {}
                }
                already_applied.0 = true;
            }

            if channeling.total_time_channeled >= swing.complete_time {
                commands.entity(ent).remove::<PerformingActionState>();
                channeling.total_time_channeled = 0.0;
            }
        }
    }
}

pub fn projectile_homing(
    mut commands: Commands,
    mut query: Query<(
        &Position,
        &mut Velocity,
        &mut Projectile,
        &ActionProjectileDetails,
    )>,
    pos_query: Query<&Position>,
) {
    for (position, mut velocity, mut projectile, details) in query.iter_mut() {
        if let Ok(position_target) = pos_query.get(projectile.target) {
            projectile.target_pos = position_target.pos;
        }
        velocity.v =
            normalized_or_zero(projectile.target_pos - position.pos) * details.projectile_speed;
    }
}

pub fn projectile_contact(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &Position,
        &Projectile,
        &ActionProjectileDetails,
        Option<&Splash>,
    )>,
    mut apply_query: Query<&mut ResolveEffectsBuffer>,
    splash_query: Query<(&Position, &Radius)>,
    origin_effect_query: Query<&OnHitEffects>,
    spatial: Res<SpatialHashTable>,
) {
    for (ent, position, projectile, details, splash_option) in query.iter_mut() {
        if position.pos.distance_to(projectile.target_pos) <= details.contact_distance {
            //Apply effects
            if let Ok(mut buffer) = apply_query.get_mut(projectile.target) {
                if let Ok(effects) = origin_effect_query.get(projectile.origin_action) {
                    for effect in effects.vec.iter() {
                        buffer.vec.push(*effect);
                    }
                }
            }

            // Handle splash
            if let Some(splash) = splash_option {
                for cell in crate::get_all_spatial_hashes_from_circle(
                    position.pos,
                    splash.radius,
                    spatial.cell_size,
                ) {
                    if let Some(potential_targets) = spatial.table.get(&cell) {
                        let mut already_effected = std::collections::HashSet::new();
                        already_effected.insert(projectile.target);

                        for potential_splash_target in potential_targets.iter() {
                            if already_effected.contains(potential_splash_target) {
                                continue;
                            }
                            if let Ok((splash_target_pos, splash_target_rad)) =
                                splash_query.get(*potential_splash_target)
                            {
                                if true_distance(
                                    projectile.target_pos,
                                    splash_target_pos.pos,
                                    splash.radius,
                                    splash_target_rad.r,
                                ) <= 0.0
                                {
                                    // Apply effects to splash targets
                                    already_effected.insert(*potential_splash_target);
                                    if let Ok(mut buffer) = apply_query.get_mut(projectile.target) {
                                        if let Ok(effects) =
                                            origin_effect_query.get(projectile.origin_action)
                                        {
                                            for effect in effects.vec.iter() {
                                                buffer.vec.push(*effect);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            commands.entity(ent).insert(DeathApproaches {
                spawn_corpse: true,
                cleanup_corpse_canvas: true,
                cleanup_time: 1.5,
            });
        }
    }
}

pub fn resolve_death(
    mut commands: Commands,
    query: Query<(
        Entity,
        &Position,
        &DeathApproaches,
        Option<&crate::graphics::Renderable>,
        Option<&crate::graphics::animation::AnimatedSprite>,
        Option<&crate::graphics::ScaleSprite>,
    )>,
) {
    for (ent, position, death, render_option, animated_sprite_option, scale_option) in query.iter() {
        if death.spawn_corpse {
            if let Some(sprite) = animated_sprite_option {
                let mut animated_sprite = crate::graphics::animation::AnimatedSprite::default();
                animated_sprite.texture = sprite.texture;


                // Negative timeout will be ignored and discarded by timeout system
                let mut timeout = death.cleanup_time;
                if !death.cleanup_corpse_canvas { timeout = -1.0 }

                let mut scale = crate::graphics::ScaleSprite(Vector2::ONE);
                if let Some(scale_existing) = scale_option {
                    scale.0 = scale_existing.0;
                }
                commands
                    .spawn()
                    .insert(crate::graphics::NewCanvasItemDirective {})
                    .insert(animated_sprite)
                    .insert(Position { pos: position.pos })
                    .insert(ExpirationTimer(timeout))
                    .insert(crate::graphics::animation::PlayAnimationDirective {
                        animation_name: "death".to_string(),
                        is_one_shot: true,
                    })
                    .insert(scale);
            }
        }
        commands.entity(ent).despawn();
        if let Some(renderable) = render_option {
            commands
                .spawn()
                .insert(CleanupCanvasItem(renderable.canvas_item_rid));
        }
    }
}

pub fn resolve_effects(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ResolveEffectsBuffer)>,
    mut damage_query: Query<&mut crate::unit::AppliedDamage>,
) {
    for (ent, mut buffer) in query.iter_mut() {
        for effect in buffer.vec.iter() {
            match effect {
                Effect::PoisonEffect(poison) => {
                    commands.entity(ent).insert(SlowPoisonDebuff {
                        effect_originator: *poison,
                        remaining_time: poison.duration,
                    });
                }
                Effect::StunEffect(stun) => {
                    commands
                        .entity(ent)
                        .insert(Stunned {
                            duration: stun.duration,
                        })
                        .remove::<PerformingActionState>();
                }
                Effect::DamageEffect(damage_instance) => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(*damage_instance);
                    }
                }
                _ => (),
            }
        }

        buffer.vec.clear();
    }
}

pub fn target_enemies(
    mut commands: Commands,
    caster_query: Query<(Entity, &UnitActions), (Without<Stunned>, Without<PerformingActionState>)>,
    mut action_query: Query<
        (
            Entity,
            &ActionRange,
            &SwingDetails,
            &mut TargetEntity,
            &TargetFlags,
        ),
        Without<Cooldown>,
    >,
    pos_query: Query<(&Position, &Radius)>,
    alignment_query: Query<&TeamAlignment>,
    debuffed_query: Query<Entity, Or<(With<Stunned>, With<SlowPoisonDebuff>)>>,
    health_query: Query<&Hitpoints>,
    neighbor_cache: Res<crate::physics::spatial_structures::SpatialNeighborsCache>,
) {
    for (ent, actions) in caster_query.iter() {
        for action in actions.vec.iter() {
            if let Ok((action_ent, range, swing_details, target_of_action, target_flags)) =
                action_query.get_mut(*action)
            {
                if let Ok((pos, rad)) = pos_query.get(ent) {
                    let mut min_dist = f32::MAX;
                    let mut cur_target = ent;
                    if let Some(neighbors) = neighbor_cache.get_neighbors(&ent, range.0) {
                        for neighbor in neighbors.iter() {
                            // Handle alignment target flags
                            let mut is_ally = false;
                            if let Ok(target_alignment) = alignment_query.get(*neighbor) {
                                if let Ok(actor_alignment) = alignment_query.get(ent) {
                                    if target_alignment.alignment == actor_alignment.alignment {
                                        is_ally = true;
                                    }
                                }
                            }
                            if !target_flags.target_allies && is_ally {
                                continue;
                            }
                            if !target_flags.target_enemies && !is_ally {
                                continue;
                            }

                            // Handle other target flags
                            if target_flags.ignore_full_health
                                && !has_full_health(neighbor, &health_query)
                            {
                                continue;
                            }
                            if target_flags.ignore_no_debuff
                                && !has_debuff(neighbor, &debuffed_query)
                            {
                                continue;
                            }

                            // Get nearest target
                            if let Ok((target_pos, target_rad)) = pos_query.get(*neighbor) {
                                let dist = crate::util::true_distance(
                                    pos.pos,
                                    target_pos.pos,
                                    rad.r,
                                    target_rad.r,
                                );
                                if dist < min_dist {
                                    min_dist = dist;
                                    cur_target = *neighbor;
                                }
                            }
                        }
                    }
                    perform_action(
                        &mut commands,
                        ent,
                        action_ent,
                        target_of_action,
                        cur_target,
                        swing_details,
                    );
                }
            }
        }
    }
}

fn has_debuff(
    ent: &Entity,
    debuff_query: &Query<Entity, Or<(With<Stunned>, With<SlowPoisonDebuff>)>>,
) -> bool {
    if let Ok(_) = debuff_query.get(*ent) {
        return true;
    }
    return false;
}

fn has_full_health(ent: &Entity, health_query: &Query<&Hitpoints>) -> bool {
    if let Ok(hp) = health_query.get(*ent) {
        return hp.hp == hp.max_hp;
    }
    return false;
}

fn perform_action(
    commands: &mut Commands,
    unit_entity: Entity,
    action_entity: Entity,
    mut target_of_action: Mut<TargetEntity>,
    action_target: Entity,
    swing_details: &SwingDetails,
) {
    commands.entity(unit_entity).insert(PerformingActionState {
        action: action_entity,
    });
    target_of_action.entity = action_target;
    commands
        .entity(action_entity)
        .insert(Cooldown(swing_details.cooldown_time));
}
