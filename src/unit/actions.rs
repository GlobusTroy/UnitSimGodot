use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    graphics::FlippableSprite,
    physics::{spatial_structures::SpatialHashTable, DeltaPhysics, Position, Radius, Velocity},
    util::true_distance,
};

use super::{effects::Effect, Hitpoints, Stunned, TeamAlignment, SlowPoisoned};

#[derive(Bundle)]
pub struct ActionBundle {
    pub channeling: ChannelingDetails,
    pub swing: SwingDetails,
    pub range: ActionRange,
    pub impact: ActionImpactType,
    pub target_flags: TargetFlags,
    pub animation: ActionAnimation,
}

impl ActionBundle {
    pub fn new(
        swing: SwingDetails,
        range: f32,
        impact: ImpactType,
        target_flags: TargetFlags,
        animation_name: String,
    ) -> ActionBundle {
        Self {
            channeling: ChannelingDetails {
                total_time_channeled: 0.0,
                effect_applied: false,
            },
            swing: swing,
            range: ActionRange(range),
            impact: ActionImpactType(impact),
            target_flags: target_flags,
            animation: ActionAnimation {
                animation_name: animation_name,
            },
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct EffectTexture(pub Rid);

#[derive(Component)]
pub struct UnitActions {
    pub vec: Vec<Entity>,
}

// Reusable action attached to a unit
#[derive(Component)]
pub struct ActionEntity {
    pub owner: Entity,
}

#[derive(Component)]
pub struct ActionAnimation {
    pub animation_name: String,
}

// Instance of an action occurring, attached to an ActionEntity
#[derive(Component)]
pub struct ActionInstanceEntity {
    pub owner: Entity,
}

#[derive(Component)]
pub struct SwingDetails {
    pub impact_time: f32,
    pub complete_time: f32,
    pub cooldown_time: f32,
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
}

#[derive(Component)]
pub struct TargetPosition {
    pub pos: Position,
}

#[derive(Component)]
pub struct ChannelingDetails {
    pub total_time_channeled: f32,
    pub effect_applied: bool,
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
    pub ignore_full_health: bool,
    pub ignore_no_debuff: bool,
    pub ignore_no_buff: bool,
    pub target_enemies: bool,
    pub target_allies: bool,
}

impl TargetFlags {
    pub fn normal_attack() -> Self {
        Self {
            ignore_full_health: false,
            ignore_no_debuff: false,
            ignore_no_buff: false,
            target_enemies: true,
            target_allies: false,
        }
    }

    pub fn heal() -> Self {
        Self {
            ignore_full_health: true,
            ignore_no_debuff: false,
            ignore_no_buff: false,
            target_enemies: false,
            target_allies: true,
        }
    }

    pub fn cleanse() -> Self {
        Self {
            ignore_full_health: false,
            ignore_no_debuff: true,
            ignore_no_buff: false,
            target_enemies: false,
            target_allies: true,
        }
    }
}

#[derive(Component)]
pub struct ActionRange(pub f32);

#[derive(Component)]
pub struct Cleave {
    pub angle_degrees: f32,
}

#[derive(Component)]
pub struct OnHitEffects {
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

pub fn performing_action_state(
    mut commands: Commands,
    query: Query<(Entity, &PerformingActionState, &Position, &Radius)>,
    mut flip_query: Query<&mut FlippableSprite>,
    mut action_query: Query<(
        Entity,
        &mut ChannelingDetails,
        &SwingDetails,
        &ActionImpactType,
        &TargetEntity,
        &OnHitEffects,
        Option<&super::projectiles::ActionProjectileDetails>,
        Option<&Cleave>,
        Option<&ActionRange>,
        Option<&super::projectiles::Splash>,
        Option<&EffectTexture>,
    )>,
    mut apply_query: Query<&mut super::effects::ResolveEffectsBuffer>,
    pos_query: Query<(&Position, &Radius)>,
    spatial: Res<SpatialHashTable>,
    delta: Res<DeltaPhysics>,
) {
    for (ent, performer, position, radius) in query.iter() {
        if let Ok((
            ent_action,
            mut channeling,
            swing,
            impact_type,
            target,
            effects,
            projectile_option,
            cleave_option,
            range_option,
            splash_option,
            texture_option,
        )) = action_query.get_mut(performer.action)
        {
            channeling.total_time_channeled += delta.seconds;

            // Handle flipping sprite
            if let Ok(mut flipper) = flip_query.get_mut(ent) {
                if !channeling.effect_applied {
                    if let Ok((caster_pos, _)) = pos_query.get(ent) {
                        if let Ok((target_pos, _)) = pos_query.get(target.entity) {
                            flipper.is_flipped = caster_pos.pos.x > target_pos.pos.x;
                            flipper.is_overriding_velocity = true;
                        }
                    }
                } else {
                    flipper.is_overriding_velocity = false;
                }
            }

            if channeling.total_time_channeled >= swing.impact_time && !channeling.effect_applied {
                match impact_type.0 {
                    ImpactType::Instant => {
                        if let Ok(mut buffer) = apply_query.get_mut(target.entity) {
                            for effect in effects.vec.iter() {
                                buffer.vec.push(*effect);
                            }
                        }

                        // Handle cleave
                        if let Some(cleave) = cleave_option {
                            if cleave.angle_degrees > 0.0 {
                                if let Some(range) = range_option {
                                    for cell in crate::get_all_spatial_hashes_from_circle(
                                        position.pos,
                                        range.0,
                                        spatial.cell_size,
                                    ) {
                                        if let Some(potential_targets) = spatial.table.get(&cell) {
                                            let mut already_effected =
                                                std::collections::HashSet::new();
                                            already_effected.insert(target.entity);
                                            if let Ok((original_target_pos, _)) =
                                                pos_query.get(target.entity)
                                            {
                                                let to_target =
                                                    original_target_pos.pos - position.pos;
                                                for potential_splash_target in
                                                    potential_targets.iter()
                                                {
                                                    if already_effected
                                                        .contains(potential_splash_target)
                                                    {
                                                        continue;
                                                    }
                                                    if let Ok((
                                                        splash_target_pos,
                                                        splash_target_rad,
                                                    )) = pos_query.get(*potential_splash_target)
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
                                                            let to_cleave = splash_target_pos.pos
                                                                - position.pos;
                                                            if to_target
                                                                .angle_to(to_cleave)
                                                                .to_degrees()
                                                                .abs()
                                                                <= cleave.angle_degrees
                                                            {
                                                                // Apply effects to cleave targets
                                                                if let Ok(mut buffer) = apply_query
                                                                    .get_mut(
                                                                        *potential_splash_target,
                                                                    )
                                                                {
                                                                    for effect in effects.vec.iter()
                                                                    {
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
                    }
                    ImpactType::Projectile => {
                        if let Some(details) = projectile_option {
                            if let Ok((target_position, _)) = pos_query.get(target.entity) {
                                let mut splash_radius = 0.0;
                                if let Some(splash) = splash_option {
                                    splash_radius = splash.radius;
                                }

                                super::projectiles::spawn_projectile(
                                    &mut commands,
                                    ent_action,
                                    position.pos,
                                    target.entity,
                                    target_position.pos,
                                    details,
                                    splash_radius,
                                );
                            }
                        }
                    }
                    _ => {}
                }

                if let Some(texture) = texture_option {
                    let mut animated_sprite = crate::graphics::animation::AnimatedSprite::default();
                    animated_sprite.texture = texture.0;
                    commands
                        .spawn()
                        .insert(crate::graphics::NewCanvasItemDirective {})
                        .insert(animated_sprite)
                        .insert(Position { pos: position.pos })
                        .insert(Velocity { v: Vector2::ZERO })
                        .insert(crate::util::ExpirationTimer(1.5))
                        .insert(crate::util::MirrorTargetPosition {})
                        .insert(TargetEntity {
                            entity: target.entity,
                        })
                        .insert(crate::graphics::ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
                        .insert(crate::graphics::animation::PlayAnimationDirective {
                            animation_name: "death".to_string(),
                            is_one_shot: true,
                        });
                }
                channeling.effect_applied = true;
            }

            if channeling.total_time_channeled >= swing.complete_time {
                commands.entity(ent).remove::<PerformingActionState>();
                channeling.total_time_channeled = 0.0;
                channeling.effect_applied = false;
            }
        }
    }
}

pub fn target_units(
    mut commands: Commands,
    caster_query: Query<(Entity, &UnitActions), (Without<Stunned>, Without<PerformingActionState>)>,
    mut action_query: Query<
        (
            Entity,
            &ActionRange,
            &SwingDetails,
            &TargetFlags,
            Option<&ActionAnimation>,
        ),
        Without<Cooldown>,
    >,
    pos_query: Query<(&Position, &Radius)>,
    alignment_query: Query<&TeamAlignment>,
    debuffed_query: Query<Entity, Or<(With<Stunned>, With<SlowPoisoned>)>>,
    health_query: Query<&Hitpoints>,
    neighbor_cache: Res<crate::physics::spatial_structures::SpatialNeighborsCache>,
) {
    for (ent, actions) in caster_query.iter() {
        for action in actions.vec.iter() {
            if let Ok((action_ent, range, swing_details, target_flags, action_animation)) =
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
                                && has_full_health(neighbor, &health_query)
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

                    if cur_target == ent {
                        continue;
                    }

                    if let Some(animation) = action_animation {
                        commands.entity(ent).insert(
                            crate::graphics::animation::PlayAnimationDirective {
                                animation_name: animation.animation_name.clone(),
                                is_one_shot: true,
                            },
                        );
                    }
                    perform_action(&mut commands, ent, action_ent, cur_target, swing_details);
                }
            }
        }
    }
}

fn has_debuff(
    ent: &Entity,
    debuff_query: &Query<Entity, Or<(With<Stunned>, With<SlowPoisoned>)>>,
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
    action_target: Entity,
    swing_details: &SwingDetails,
) {
    commands.entity(unit_entity).insert(PerformingActionState {
        action: action_entity,
    });
    commands
        .entity(action_entity)
        .insert(TargetEntity {
            entity: action_target,
        })
        .insert(Cooldown(swing_details.cooldown_time));
}
