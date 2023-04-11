use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::physics::{Position, Radius, Velocity};

#[derive(Component)]
pub struct Projectile {
    pub target: Entity,
    pub target_pos: Vector2,
    pub origin_action: Entity,
}

#[derive(Component, Copy, Clone)]
pub struct ActionProjectileDetails {
    pub projectile_speed: f32,
    pub projectile_scale: f32,
    pub projectile_texture: Rid,
    pub contact_distance: f32,
}

#[derive(Component)]
pub struct Splash {
    pub radius: f32,
}

#[derive(Component)]
pub struct DamageOverride {
    pub damage: f32,
}

pub fn spawn_projectile(
    commands: &mut Commands,
    origin_action: Entity,
    origin_pos: Vector2,
    target: Entity,
    target_pos: Vector2,
    details: &ActionProjectileDetails,
    splash_radius: f32,
) {
    commands
        .spawn()
        .insert(Projectile {
            target: target,
            target_pos: target_pos,
            origin_action: origin_action,
        })
        .insert(*details)
        .insert(Position { pos: origin_pos })
        .insert(Velocity { v: Vector2::ZERO })
        .insert(Splash {
            radius: splash_radius,
        })
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

pub fn projectile_homing(
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
        velocity.v = crate::util::normalized_or_zero(projectile.target_pos - position.pos)
            * details.projectile_speed;
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
        Option<&DamageOverride>,
    )>,
    mut apply_query: Query<&mut crate::effects::ResolveEffectsBuffer>,
    splash_query: Query<(&Position, &Radius)>,
    origin_effect_query: Query<&super::actions::OnHitEffects>,
    spatial: Res<crate::physics::spatial_structures::SpatialHashTable>,
    mut events: ResMut<crate::event::EventQueue>,
) {
    for (ent, position, projectile, details, splash_option, damage_override_option) in
        query.iter_mut()
    {
        if position.pos.distance_to(projectile.target_pos) <= details.contact_distance {
            // Event Cue
            events
                .0
                .push(crate::EventCue::Audio(crate::event::AudioCue {
                    event: "impact".to_string(),
                    location: position.pos,
                    texture: details.projectile_texture,
                }));

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
                                if crate::util::true_distance(
                                    projectile.target_pos,
                                    splash_target_pos.pos,
                                    splash.radius,
                                    splash_target_rad.r,
                                ) <= 0.0
                                {
                                    // Apply effects to splash targets
                                    already_effected.insert(*potential_splash_target);
                                    if let Ok(mut buffer) =
                                        apply_query.get_mut(*potential_splash_target)
                                    {
                                        if let Ok(effects) =
                                            origin_effect_query.get(projectile.origin_action)
                                        {
                                            for effect in effects.vec.iter() {
                                                if let super::effects::Effect::DamageEffect(dmg) =
                                                    effect
                                                {
                                                    buffer.vec.push(
                                                        super::effects::Effect::DamageEffect(
                                                            super::DamageInstance {
                                                                damage: dmg.damage,
                                                                delay: dmg.delay,
                                                                damage_type:
                                                                    super::DamageType::Magic,
                                                                originator: dmg.originator,
                                                            },
                                                        ),
                                                    );
                                                } else {
                                                    buffer.vec.push(*effect);
                                                }
                                            }
                                        } else {
                                            if let Some(damage_override) = damage_override_option {
                                                buffer.vec.push(
                                                    super::effects::Effect::DamageEffect(
                                                        super::DamageInstance {
                                                            damage: damage_override.damage,
                                                            delay: 0.0,
                                                            damage_type: super::DamageType::Magic,
                                                            originator: projectile.origin_action,
                                                        },
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            commands.entity(ent).insert(super::DeathApproaches {
                spawn_corpse: true,
                cleanup_corpse_canvas: true,
                cleanup_time: 1.5,
            });
        }
    }
}
