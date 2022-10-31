use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    graphics::{
        animation::{AnimatedSprite, PlayAnimationDirective},
        particles::NewParticleEffectDirective,
        CleanupCanvasItem, FlippableSprite, NewCanvasItemDirective, Renderable, ScaleSprite,
    },
    physics::{
        spatial_structures::SpatialNeighborsCache, DeltaPhysics, Position, Radius, Velocity,
    },
    util::{normalized_or_zero, true_distance, ExpirationTimer},
};

#[derive(Debug, Clone)]
pub struct UnitBlueprint {
    pub radius: f32,
    pub mass: f32,
    pub movespeed: f32,
    pub acceleration: f32,
    pub hitpoints: f32,
    pub texture: Rid,
    pub weapons: Vec<Weapon>,
}

impl UnitBlueprint {
    pub fn new(
        texture: Rid,
        hitpoints: f32,
        radius: f32,
        mass: f32,
        movespeed: f32,
        acceleration: f32,
    ) -> Self {
        Self {
            radius: radius,
            mass: mass,
            movespeed: movespeed,
            acceleration: acceleration,
            texture: texture,
            hitpoints: hitpoints,
            weapons: Vec::new(),
        }
    }

    pub fn add_weapon(&mut self, weapon: Weapon) {
        self.weapons.push(weapon);
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum TeamValue {
    NeutralPassive,
    NeutralHostile,
    Team(usize),
}

#[derive(Component)]
pub struct TeamAlignment {
    pub alignment: TeamValue,
}

#[derive(Component)]
pub struct Hitpoints {
    pub max_hp: f32,
    pub hp: f32,
}

pub struct DamageInstance {
    pub damage: f32,
    pub delay: f32,
}

#[derive(Component)]
pub struct AppliedDamage {
    pub damages: Vec<DamageInstance>,
}

#[derive(Component, Clone, Debug, Copy)]
pub struct MeleeWeapon {
    pub damage: f32,
    pub range: f32,

    pub cooldown_time: f32,
    pub impact_time: f32,
    pub full_swing_time: f32,

    pub time_until_weapon_cooled: f32,
}

#[derive(Component, Clone, Debug, Copy)]
pub struct ProjectileWeapon {
    pub damage: f32,
    pub range: f32,

    pub cooldown_time: f32,
    pub impact_time: f32,
    pub projectile_speed: f32,
    pub projectile_scale: f32,
    pub full_swing_time: f32,
    pub projectile_texture: Rid,

    pub time_until_weapon_cooled: f32,
}

#[derive(Component)]
pub struct TargetedProjectile {
    pub target: Entity,
    pub target_pos: Vector2,
    pub contact_dist: f32,
    pub originating_weapon: ProjectileWeapon,
}

#[derive(Component)]
pub struct Stunned {
    pub duration: f32,
}

#[derive(Component)]
pub struct AttackTargetDirective {
    pub target: Entity,
}

#[derive(Clone, Debug)]
pub enum Weapon {
    Melee(MeleeWeapon),
    Projectile(ProjectileWeapon),
}

#[derive(Component)]
pub struct Attacking {
    pub weapon: Weapon,
    pub target: Entity,
    pub channeling_time: f32,
}

#[derive(Component)]
pub struct AttackEnemyBehavior {}

pub fn execute_attack_target_directive(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &AttackTargetDirective,
            &Position,
            &Radius,
            Option<&mut MeleeWeapon>,
            Option<&mut ProjectileWeapon>,
        ),
        Without<Stunned>,
    >,
    mut target: Query<(&Position, &Radius)>,
) {
    for (entity, directive, position, radius, melee_option, projectile_option) in query.iter_mut() {
        if let Ok((position2, radius2)) = target.get_mut(directive.target) {
            if let Some(mut weapon) = melee_option {
                if true_distance(position.pos, position2.pos, radius.r, radius2.r) < weapon.range {
                    if weapon.time_until_weapon_cooled <= 0.0 {
                        // Melee Attack
                        commands.entity(entity).insert(PlayAnimationDirective {
                            animation_name: "attack".to_string(),
                            is_one_shot: true,
                        });
                        commands
                            .entity(entity)
                            .insert(Stunned {
                                duration: weapon.full_swing_time,
                            })
                            .insert(Attacking {
                                weapon: Weapon::Melee(weapon.clone()),
                                target: directive.target,
                                channeling_time: 0.0,
                            });
                        weapon.time_until_weapon_cooled = weapon.cooldown_time;
                    }
                }
            }
            if let Some(mut weapon) = projectile_option {
                if true_distance(position.pos, position2.pos, radius.r, radius2.r) < weapon.range {
                    if weapon.time_until_weapon_cooled <= 0.0 {
                        // Projectile Attack
                        commands.entity(entity).insert(PlayAnimationDirective {
                            animation_name: "attack".to_string(),
                            is_one_shot: true,
                        });
                        commands
                            .entity(entity)
                            .insert(Stunned {
                                duration: weapon.full_swing_time,
                            })
                            .insert(Attacking {
                                weapon: Weapon::Projectile(weapon.clone()),
                                target: directive.target,
                                channeling_time: 0.0,
                            });
                        weapon.time_until_weapon_cooled = weapon.cooldown_time;
                    }
                }
            }
        }
        commands.entity(entity).remove::<AttackTargetDirective>();
    }
}

pub fn attacking_state(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Attacking, Option<&mut FlippableSprite>)>,
    pos_query: Query<&Position>,
    mut target_query: Query<&mut AppliedDamage>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut attack, flippable_option) in query.iter_mut() {
        let weapon_clone = attack.weapon.clone();
        if let Weapon::Melee(weapon) = weapon_clone {
            // Impact hit -> apply damage
            if attack.channeling_time < weapon.impact_time
                && attack.channeling_time + delta.seconds >= weapon.impact_time
            {
                if let Ok(mut damage_holder) = target_query.get_mut(attack.target) {
                    damage_holder.damages.push(DamageInstance {
                        damage: weapon.damage,
                        delay: 0.,
                    });
                }
            }
            // End attacking state
            if attack.channeling_time < weapon.full_swing_time
                && attack.channeling_time + delta.seconds >= weapon.full_swing_time
            {
                commands.entity(entity).remove::<Attacking>();
                commands.entity(entity).remove::<Stunned>();
            }
        } else if let Weapon::Projectile(weapon) = weapon_clone {
            if attack.channeling_time < weapon.impact_time
                && attack.channeling_time + delta.seconds >= weapon.impact_time
            {
                if let Ok(position) = pos_query.get(entity) {
                    commands
                        .spawn()
                        .insert(Position { pos: position.pos })
                        .insert(Velocity { v: Vector2::ZERO })
                        .insert(TargetedProjectile {
                            target: attack.target,
                            target_pos: position.pos,
                            originating_weapon: weapon,
                            contact_dist: 24.,
                        })
                        .insert(NewCanvasItemDirective {})
                        .insert(AnimatedSprite {
                            texture: weapon.projectile_texture,
                            animation_name: "fly".to_string(),
                            animation_index: 0,
                            animation_speed: 60.,
                            animation_time_since_change: 0.,
                            animation_length: 100,
                            is_one_shot: false,
                        })
                        .insert(PlayAnimationDirective {
                            animation_name: "fly".to_string(),
                            is_one_shot: false,
                        })
                        .insert(ScaleSprite(Vector2 {
                            x: weapon.projectile_scale,
                            y: weapon.projectile_scale,
                        }));
                }
            }
            // End attacking state
            if attack.channeling_time < weapon.full_swing_time
                && attack.channeling_time + delta.seconds >= weapon.full_swing_time
            {
                commands.entity(entity).remove::<Attacking>();
                commands.entity(entity).remove::<Stunned>();
            }
        }
        attack.channeling_time += delta.seconds;

        if let Some(mut flipper) = flippable_option {
            if let Ok(attacker_pos) = pos_query.get(entity) {
                if let Ok(target_pos) = pos_query.get(attack.target) {
                    flipper.is_flipped = attacker_pos.pos.x > target_pos.pos.x;
                }
            }
        }
    }
}

pub fn update_targeted_projectiles(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut TargetedProjectile,
        &Position,
        &mut Velocity,
        Option<&Renderable>,
    )>,
    pos_query: Query<&Position>,
    mut damage_query: Query<&mut AppliedDamage>,
) {
    for (entity, mut projectile, position, mut velocity, renderable_option) in query.iter_mut() {
        if let Ok(position_target) = pos_query.get(projectile.target) {
            projectile.target_pos = position_target.pos;
        }

        velocity.v = normalized_or_zero(projectile.target_pos - position.pos)
            * projectile.originating_weapon.projectile_speed;
        if position.pos.distance_squared_to(projectile.target_pos) <= projectile.contact_dist {
            if let Ok(mut damage_container) = damage_query.get_mut(projectile.target) {
                damage_container.damages.push(DamageInstance {
                    damage: projectile.originating_weapon.damage,
                    delay: 0.0,
                });
            }
            commands.entity(entity).despawn();
            if let Some(renderable) = renderable_option {
                commands
                    .spawn()
                    .insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
        }
    }
}

pub fn attack_enemy_behavior(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &TeamAlignment,
            &Position,
            &Radius,
            Option<&MeleeWeapon>,
            Option<&ProjectileWeapon>,
        ),
        (With<AttackEnemyBehavior>, Without<Stunned>),
    >,
    target_query: Query<(&TeamAlignment, &Position, &Radius)>,
    spatial: Res<SpatialNeighborsCache>,
) {
    for (entity, alignment, position, radius, melee_option, projectile_option) in query.iter() {
        if let Some(melee) = melee_option {
            // Melee attack
            if melee.time_until_weapon_cooled > 0.0 {
                continue;
            }

            let mut min_dist = f32::MAX;
            if let Some(targets) = spatial.get_neighbors(&entity, melee.range) {
                for target in targets {
                    if let Ok((alignment_target, position_target, radius_target)) =
                        target_query.get(target)
                    {
                        if alignment_target.alignment != alignment.alignment
                            && alignment_target.alignment != TeamValue::NeutralPassive
                        {
                            let dist = true_distance(
                                position.pos,
                                position_target.pos,
                                radius.r,
                                radius_target.r,
                            );
                            if dist < min_dist {
                                min_dist = dist;
                                commands
                                    .entity(entity)
                                    .insert(AttackTargetDirective { target: target });
                            }
                        }
                    }
                }
            }
        }

        if let Some(projectile) = projectile_option {
            // Projectile attack
            if projectile.time_until_weapon_cooled > 0.0 {
                continue;
            }

            let mut min_dist = f32::MAX;
            if let Some(targets) = spatial.get_neighbors(&entity, projectile.range) {
                for target in targets {
                    if let Ok((alignment_target, position_target, radius_target)) =
                        target_query.get(target)
                    {
                        if alignment_target.alignment != alignment.alignment
                            && alignment_target.alignment != TeamValue::NeutralPassive
                        {
                            let dist = true_distance(
                                position.pos,
                                position_target.pos,
                                radius.r,
                                radius_target.r,
                            );
                            if dist < min_dist {
                                min_dist = dist;
                                commands
                                    .entity(entity)
                                    .insert(AttackTargetDirective { target: target });
                            }
                        }
                    }
                }
            }
        }
        // Other weapon types in else block here
    }
}

pub fn apply_damages(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut AppliedDamage,
        &mut Hitpoints,
        Option<&Position>,
        Option<&Renderable>,
        Option<&AnimatedSprite>,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut damages, mut hitpoints, position_option, renderable_option, sprite_option) in
        query.iter_mut()
    {
        let mut i = 0;
        while i < damages.damages.len() && !damages.damages.is_empty() {
            let mut damage = damages.damages.get_mut(i).unwrap();
            damage.delay -= delta.seconds;
            if damage.delay <= 0.0 {
                hitpoints.hp -= damage.damage;
                damages.damages.remove(i);
            } else {
                i += 1;
            }
        }
        if hitpoints.hp <= 0.0 {
            commands.entity(entity).despawn();
            if let Some(sprite) = sprite_option {
                if let Some(position) = position_option {
                    let mut animated_sprite = AnimatedSprite::default();
                    animated_sprite.texture = sprite.texture;
                    commands
                        .spawn()
                        .insert(NewCanvasItemDirective {})
                        .insert(animated_sprite)
                        .insert(Position { pos: position.pos })
                        .insert(PlayAnimationDirective {
                            animation_name: "death".to_string(),
                            is_one_shot: true,
                        });
                    // commands
                    //     .spawn()
                    //     .insert(NewParticleEffectDirective {
                    //         effect_name: "deathsplash".to_string(),
                    //         position: position.pos
                    //     });
                }
            }
            if let Some(renderable) = renderable_option {
                commands
                    .spawn()
                    .insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
        }
    }
}

pub fn melee_weapon_cooldown(mut query: Query<&mut MeleeWeapon>, delta: Res<DeltaPhysics>) {
    for mut weapon in query.iter_mut() {
        if weapon.time_until_weapon_cooled < 0.0 {
            continue;
        }
        weapon.time_until_weapon_cooled -= delta.seconds;
    }
}

pub fn projectile_weapon_cooldown(
    mut query: Query<&mut ProjectileWeapon>,
    delta: Res<DeltaPhysics>,
) {
    for mut weapon in query.iter_mut() {
        if weapon.time_until_weapon_cooled < 0.0 {
            continue;
        }
        weapon.time_until_weapon_cooled -= delta.seconds;
    }
}

pub fn remove_stuns(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Stunned)>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut stunned) in query.iter_mut() {
        stunned.duration -= delta.seconds;
        if stunned.duration <= 0.0 {
            commands.entity(entity).remove::<Stunned>();
        }
    }
}
