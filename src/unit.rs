use bevy_ecs::prelude::*;
use gdnative::prelude::*;

pub mod abilities;
pub mod actions;
use actions::*;

use crate::{
    boid::BoidParams,
    graphics::{
        animation::{AnimatedSprite, PlayAnimationDirective},
        CleanupCanvasItem, FlippableSprite, NewCanvasItemDirective, Renderable, ScaleSprite,
    },
    physics::{
        spatial_structures::{SpatialHashTable, SpatialNeighborsCache},
        DeltaPhysics, Position, Radius, Velocity,
    },
    util::{normalized_or_zero, true_distance, ExpirationTimer},
};

use self::abilities::*;

#[derive(Debug, Clone)]
pub struct UnitBlueprint {
    pub radius: f32,
    pub mass: f32,
    pub movespeed: f32,
    pub acceleration: f32,
    pub hitpoints: f32,
    pub texture: Rid,
    pub weapons: Vec<Weapon>,
    pub abilities: Vec<UnitAbility>,
    pub armor: f32,
    pub magic_resist: f32,
}

impl UnitBlueprint {
    pub fn new(
        texture: Rid,
        hitpoints: f32,
        radius: f32,
        mass: f32,
        movespeed: f32,
        acceleration: f32,
        armor: f32,
        magic_resist: f32,
    ) -> Self {
        Self {
            radius: radius,
            mass: mass,
            movespeed: movespeed,
            acceleration: acceleration,
            texture: texture,
            hitpoints: hitpoints,
            weapons: Vec::new(),
            abilities: Vec::new(),
            armor: armor,
            magic_resist: magic_resist,
        }
    }

    pub fn add_weapon(&mut self, weapon: Weapon) {
        self.weapons.push(weapon);
    }

    pub fn add_ability(&mut self, ability: UnitAbility) {
        self.abilities.push(ability);
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum TeamValue {
    NeutralPassive,
    NeutralHostile,
    Team(usize),
}

#[derive(Component, Copy, Clone)]
pub struct StunOnHitEffect {
    pub duration: f32,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DamageType {
    Normal,
    Poison,
    Magic,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct DamageInstance {
    pub damage: f32,
    pub delay: f32,
    pub damage_type: DamageType,
}

#[derive(Component)]
pub struct AppliedDamage {
    pub damages: Vec<DamageInstance>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SlowPoisonDebuff {
    pub remaining_time: f32,
    pub effect_originator: SlowPoisonAttack,
}

#[derive(Component, Clone, Debug, Copy)]
pub struct MeleeWeapon {
    pub damage: f32,
    pub range: f32,

    pub cooldown_time: f32,
    pub impact_time: f32,
    pub full_swing_time: f32,

    pub time_until_weapon_cooled: f32,
    pub stun_duration: f32,
    pub cleave_degrees: f32,
}

#[derive(Component, Clone, Debug, Copy)]
pub struct RadiusWeapon {
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
    pub splash_radius: f32,
    pub time_until_weapon_cooled: f32,
}

#[derive(Component)]
pub struct TargetedProjectile {
    pub target: Entity,
    pub target_pos: Vector2,
    pub contact_dist: f32,
    pub poison_option: Option<SlowPoisonAttack>,
    pub originating_weapon: ProjectileWeapon,
}

#[derive(Component)]
pub struct Channeling {
    pub duration: f32,
}

#[derive(Component)]
pub struct Stunned {
    pub duration: f32,
}

#[derive(Component)]
pub struct AttackTargetDirective {
    pub target: Entity,
}

#[derive(Component)]
pub struct CleanseAllyDirective {
    pub target: Entity,
}

#[derive(Component)]
pub struct HealAllyDirective {
    pub target: Entity,
}

#[derive(Clone, Debug)]
pub enum Weapon {
    Melee(MeleeWeapon),
    Projectile(ProjectileWeapon),
    Radius(RadiusWeapon),
}

#[derive(Component)]
pub struct Attacking {
    pub weapon: Weapon,
    pub target: Entity,
    pub channeling_time: f32,
}

#[derive(Component)]
pub struct Casting {
    pub ability: UnitAbility,
    pub target: Entity,
    pub channeling_time: f32,
}

#[derive(Clone, Copy, Component)]
pub struct Armor {
    pub armor: f32,
}

#[derive(Clone, Copy, Component)]
pub struct MagicArmor {
    pub percent_resist: f32,
}

#[derive(Component)]
pub struct AttackEnemyBehavior {}

#[derive(Component)]
pub struct HealAllyBehavior {}

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
            Option<&mut RadiusWeapon>,
        ),
        (Without<Channeling>, Without<Stunned>),
    >,
    mut target: Query<(&Position, &Radius)>,
) {
    for (
        entity,
        directive,
        position,
        radius,
        melee_option,
        projectile_option,
        radius_weapon_option,
    ) in query.iter_mut()
    {
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
                            .insert(Channeling {
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
                            .insert(Channeling {
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
            if let Some(mut weapon) = radius_weapon_option {
                if true_distance(position.pos, position2.pos, radius.r, radius2.r) < weapon.range {
                    if weapon.time_until_weapon_cooled <= 0.0 {
                        // Melee Attack
                        commands.entity(entity).insert(PlayAnimationDirective {
                            animation_name: "attack".to_string(),
                            is_one_shot: true,
                        });
                        commands
                            .entity(entity)
                            .insert(Channeling {
                                duration: weapon.full_swing_time,
                            })
                            .insert(Attacking {
                                weapon: Weapon::Radius(weapon.clone()),
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

pub fn execute_cleanse_ally_directive(
    mut commands: Commands,
    mut query: Query<
        (Entity, &CleanseAllyDirective, &mut CleanseAbility),
        (Without<Channeling>, Without<Stunned>),
    >,
    mut target: Query<Entity, Or<(With<Stunned>, With<SlowPoisonDebuff>)>>,
) {
    for (entity, directive, mut cleanse) in query.iter_mut() {
        if let Ok(_) = target.get_mut(directive.target) {
            if cleanse.time_until_cleanse_cooled <= 0.0 {
                // Cleanse
                commands.entity(entity).insert(PlayAnimationDirective {
                    animation_name: "cast".to_string(),
                    is_one_shot: false,
                });
                commands
                    .entity(entity)
                    .insert(Channeling {
                        duration: cleanse.swing_time,
                    })
                    .insert(Casting {
                        ability: UnitAbility::Cleanse(cleanse.clone()),
                        target: directive.target,
                        channeling_time: 0.0,
                    });
                cleanse.time_until_cleanse_cooled = cleanse.cooldown;
            }
            commands.entity(entity).remove::<CleanseAllyDirective>();
        }
    }
}

pub fn execute_heal_ally_directive(
    mut commands: Commands,
    mut query: Query<
        (Entity, &HealAllyDirective, &mut HealAbility),
        (Without<Channeling>, Without<Stunned>),
    >,
    mut target: Query<Entity>,
) {
    for (entity, directive, mut heal) in query.iter_mut() {
        if let Ok(_) = target.get_mut(directive.target) {
            if heal.time_until_cooled <= 0.0 {
                // Cleanse
                commands.entity(entity).insert(PlayAnimationDirective {
                    animation_name: "cast".to_string(),
                    is_one_shot: false,
                });
                commands
                    .entity(entity)
                    .insert(Channeling {
                        duration: heal.swing_time,
                    })
                    .insert(Casting {
                        ability: UnitAbility::Heal(*heal),
                        target: directive.target,
                        channeling_time: 0.0,
                    });
                heal.time_until_cooled = heal.cooldown;
            }
            commands.entity(entity).remove::<HealAllyDirective>();
        }
    }
}

pub fn attacking_state(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut Attacking,
        Option<&mut FlippableSprite>,
        Option<&SlowPoisonAttack>,
    )>,
    mut poisoned_query: Query<&mut SlowPoisonDebuff>,
    mut boid_query: Query<&mut BoidParams>,
    pos_query: Query<&Position>,
    alignment_query: Query<&TeamAlignment>,
    mut target_query: Query<&mut AppliedDamage>,
    mut stunned_query: Query<&mut Channeling>,
    spatial: Res<SpatialNeighborsCache>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut attack, flippable_option, slow_poison_option) in query.iter_mut() {
        let weapon_clone = attack.weapon.clone();
        if let Weapon::Melee(weapon) = weapon_clone {
            // Impact hit -> apply damage
            if attack.channeling_time < weapon.impact_time
                && attack.channeling_time + delta.seconds >= weapon.impact_time
            {
                let mut swinger_pos = Vector2::ZERO;
                if let Ok(swinger_position) = pos_query.get(entity) {
                    swinger_pos = swinger_position.pos;
                }

                let mut main_target_pos = Vector2::ZERO;
                if let Ok(target_position) = pos_query.get(attack.target) {
                    main_target_pos = target_position.pos;
                }

                let mut alignment = TeamValue::NeutralHostile;
                if let Ok(swinger_alignment) = alignment_query.get(entity) {
                    alignment = swinger_alignment.alignment;
                }

                let vec_to_main_target = swinger_pos.direction_to(main_target_pos);

                let mut targets = vec![attack.target];
                if let Some(neighbors) = spatial.get_neighbors(&entity, weapon.range) {
                    for neighbor in neighbors.iter() {
                        if *neighbor == entity {
                            continue;
                        }
                        if *neighbor == attack.target {
                            continue;
                        }
                        if let Ok(team_alignment_target) = alignment_query.get(*neighbor) {
                            if team_alignment_target.alignment == alignment {
                                continue;
                            }
                        }
                        if let Ok(neighbor_position) = pos_query.get(*neighbor) {
                            let vec_to_neighbor = swinger_pos.direction_to(neighbor_position.pos);
                            if vec_to_main_target.angle_to(vec_to_neighbor).to_degrees()
                                <= weapon.cleave_degrees
                            {
                                targets.push(*neighbor);
                            }
                        }
                    }
                }

                for target in targets.iter() {
                    if let Ok(mut damage_holder) = target_query.get_mut(*target) {
                        damage_holder.damages.push(DamageInstance {
                            damage: weapon.damage,
                            delay: 0.,
                            damage_type: DamageType::Normal,
                        });

                        if let Some(poison) = slow_poison_option {
                            if let Ok(mut poison_effect) = poisoned_query.get_mut(*target) {
                                if let Ok(mut boid) = boid_query.get_mut(attack.target) {
                                    (*boid).max_speed /=
                                        poison_effect.effect_originator.speed_multiplier;
                                    (*boid).max_speed *= poison.speed_multiplier;
                                    poison_effect.remaining_time = poison.duration;
                                    poison_effect.effect_originator = *poison;
                                }
                            } else {
                                // Guardrail = damage_holder
                                commands.entity(attack.target).insert(SlowPoisonDebuff {
                                    remaining_time: poison.duration,
                                    effect_originator: *poison,
                                });
                                if let Ok(mut boid) = boid_query.get_mut(*target) {
                                    boid.max_speed *= poison.speed_multiplier;
                                }
                            }
                        }

                        if weapon.stun_duration > 0.0 {
                            if let Ok(mut stunned) = stunned_query.get_mut(*target) {
                                stunned.duration = stunned.duration.max(weapon.stun_duration);
                            } else {
                                // commands.entity(entity) Panics if entity doesn't exist
                                // We know it does here because let Ok(damage_holder) returned a value
                                commands
                                    .entity(*target)
                                    .insert(Stunned {
                                        duration: weapon.stun_duration,
                                    })
                                    .insert(PlayAnimationDirective {
                                        animation_name: "stun".to_string(),
                                        is_one_shot: true,
                                    });
                            }
                        }
                    }
                }
            }
            // End attacking state
            if attack.channeling_time < weapon.full_swing_time
                && attack.channeling_time + delta.seconds >= weapon.full_swing_time
            {
                commands.entity(entity).remove::<Attacking>();
                commands.entity(entity).remove::<Channeling>();
            }
        } else if let Weapon::Projectile(weapon) = weapon_clone {
            if attack.channeling_time < weapon.impact_time
                && attack.channeling_time + delta.seconds >= weapon.impact_time
            {
                let mut poison = None;
                if let Some(slow_poison) = slow_poison_option {
                    poison = Some(*slow_poison);
                }
                if let Ok(position) = pos_query.get(entity) {
                    commands
                        .spawn()
                        .insert(Position { pos: position.pos })
                        .insert(Velocity { v: Vector2::ZERO })
                        .insert(TargetedProjectile {
                            target: attack.target,
                            target_pos: position.pos,
                            originating_weapon: weapon,
                            contact_dist: 12.,
                            poison_option: poison,
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
                commands.entity(entity).remove::<Channeling>();
            }
        } else if let Weapon::Radius(weapon) = weapon_clone {
            // Impact hit -> apply damage
            if attack.channeling_time < weapon.impact_time
                && attack.channeling_time + delta.seconds >= weapon.impact_time
            {
                if let Ok(mut damage_holder) = target_query.get_mut(attack.target) {
                    damage_holder.damages.push(DamageInstance {
                        damage: weapon.damage,
                        delay: 0.,
                        damage_type: DamageType::Normal,
                    });
                }
            }
            // End attacking state
            if attack.channeling_time < weapon.full_swing_time
                && attack.channeling_time + delta.seconds >= weapon.full_swing_time
            {
                commands.entity(entity).remove::<Attacking>();
                commands.entity(entity).remove::<Channeling>();
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
        Option<&AnimatedSprite>,
        Option<&Renderable>,
        Option<&ScaleSprite>,
    )>,
    pos_query: Query<&Position>,
    splash_query: Query<(&Position, &Radius)>,
    mut poisoned_query: Query<&mut SlowPoisonDebuff>,
    mut boid_query: Query<&mut BoidParams>,
    mut damage_query: Query<&mut AppliedDamage>,
    spatial: Res<SpatialHashTable>,
) {
    for (
        entity,
        mut projectile,
        position,
        mut velocity,
        animated_sprite_option,
        renderable_option,
        scale_option,
    ) in query.iter_mut()
    {
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
                    damage_type: DamageType::Normal,
                });
            }

            if let Some(poison) = projectile.poison_option {
                if let Ok(mut poison_effect) = poisoned_query.get_mut(projectile.target) {
                    if let Ok(mut boid) = boid_query.get_mut(projectile.target) {
                        (*boid).max_speed /= poison_effect.effect_originator.speed_multiplier;
                        (*boid).max_speed *= poison.speed_multiplier;
                        poison_effect.remaining_time = poison.duration;
                        poison_effect.effect_originator = poison;
                    }
                } else {
                    // Guardrail against despawned entity
                    if let Ok(_) = pos_query.get(projectile.target) {
                        commands.entity(projectile.target).insert(SlowPoisonDebuff {
                            remaining_time: poison.duration,
                            effect_originator: poison,
                        });
                        if let Ok(mut boid) = boid_query.get_mut(projectile.target) {
                            boid.max_speed *= poison.speed_multiplier;
                        }
                    }
                }
            }

            if projectile.originating_weapon.splash_radius > 0.0 {
                for spatial_hash in
                    crate::physics::spatial_structures::get_all_spatial_hashes_from_circle(
                        projectile.target_pos,
                        projectile.originating_weapon.splash_radius,
                        spatial.cell_size,
                    )
                    .iter()
                {
                    if let Some(entities) = spatial.table.get(&spatial_hash) {
                        for entity_splashed in entities.iter() {
                            if *entity_splashed == projectile.target {
                                continue;
                            }
                            if entity.eq(entity_splashed) {
                                continue;
                            }
                            if let Ok((target_pos, target_rad)) = splash_query.get(*entity_splashed)
                            {
                                // Unit within splash radius
                                if true_distance(
                                    target_pos.pos,
                                    projectile.target_pos,
                                    projectile.originating_weapon.splash_radius,
                                    target_rad.r,
                                ) <= 0.0
                                {
                                    if let Ok(mut damage_container) =
                                        damage_query.get_mut(*entity_splashed)
                                    {
                                        damage_container.damages.push(DamageInstance {
                                            damage: projectile.originating_weapon.damage,
                                            delay: 0.0,
                                            damage_type: DamageType::Magic,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            commands.entity(entity).despawn();
            if let Some(renderable) = renderable_option {
                commands
                    .spawn()
                    .insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
            if let Some(sprite) = animated_sprite_option {
                let mut animated_sprite = AnimatedSprite::default();
                animated_sprite.texture = sprite.texture;
                let mut scale = ScaleSprite(Vector2::ONE);
                if let Some(scale_existing) = scale_option {
                    scale.0 = scale_existing.0;
                }
                commands
                    .spawn()
                    .insert(NewCanvasItemDirective {})
                    .insert(animated_sprite)
                    .insert(Position { pos: position.pos })
                    .insert(ExpirationTimer(1.5))
                    .insert(PlayAnimationDirective {
                        animation_name: "death".to_string(),
                        is_one_shot: true,
                    })
                    .insert(scale);
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
        (
            With<AttackEnemyBehavior>,
            Without<Channeling>,
            Without<Stunned>,
        ),
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

pub fn heal_ally_behavior(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &TeamAlignment,
            Option<&CleanseAbility>,
            Option<&HealAbility>,
        ),
        (
            With<HealAllyBehavior>,
            Without<Channeling>,
            Without<Stunned>,
        ),
    >,
    cleanse_query: Query<Entity, Or<(With<SlowPoisonDebuff>, With<Stunned>)>>,
    heal_query: Query<(Entity, &Hitpoints)>,
    target_query: Query<&TeamAlignment>,
    spatial: Res<SpatialNeighborsCache>,
) {
    for (entity, alignment, cleanse_option, heal_option) in query.iter() {
        if let Some(heal) = heal_option {
            // Cleanse on cooldown
            if heal.time_until_cooled > 0.0 {
                continue;
            }

            if let Some(targets) = spatial.get_neighbors(&entity, heal.range) {
                for target in targets {
                    if let Ok(alignment_target) = target_query.get(target) {
                        if alignment_target.alignment == alignment.alignment {
                            if let Ok((entity_healable, healable)) = heal_query.get(target) {
                                if healable.hp < healable.max_hp {
                                    commands.entity(entity).insert(HealAllyDirective {
                                        target: entity_healable,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(cleanse) = cleanse_option {
            if cleanse.time_until_cleanse_cooled > 0.0 {
                continue;
            }

            if let Some(targets) = spatial.get_neighbors(&entity, cleanse.range) {
                for target in targets {
                    if let Ok(alignment_target) = target_query.get(target) {
                        if alignment_target.alignment == alignment.alignment {
                            if let Ok(entity_cleansable) = cleanse_query.get(target) {
                                commands.entity(entity).insert(CleanseAllyDirective {
                                    target: entity_cleansable,
                                });
                            }
                        }
                    }
                }
            }
        }
        // Other spell types
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
        Option<&Armor>,
        Option<&MagicArmor>,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (
        entity,
        mut damages,
        mut hitpoints,
        position_option,
        renderable_option,
        sprite_option,
        armor_option,
        magic_armor_option,
    ) in query.iter_mut()
    {
        let mut i = 0;
        while i < damages.damages.len() && !damages.damages.is_empty() {
            let mut damage = damages.damages.get_mut(i).unwrap();
            damage.delay -= delta.seconds;
            if damage.delay <= 0.0 {
                if damage.damage_type == DamageType::Normal {
                    if let Some(armor) = armor_option {
                        damage.damage = (damage.damage - armor.armor).max(1.0);
                    }
                } else if damage.damage_type == DamageType::Magic {
                    if let Some(magic_armor) = magic_armor_option {
                        damage.damage *= 1. - magic_armor.percent_resist;
                    }
                }
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

pub fn ability_cooldowns(
    mut query: Query<(Option<&mut CleanseAbility>, Option<&mut HealAbility>)>,
    delta: Res<DeltaPhysics>,
) {
    for (cleanse_option, heal_option) in query.iter_mut() {
        if let Some(mut cleanse) = cleanse_option {
            if cleanse.time_until_cleanse_cooled < 0.0 {
                continue;
            }
            cleanse.time_until_cleanse_cooled -= delta.seconds;
        } else if let Some(mut heal) = heal_option {
            if heal.time_until_cooled < 0.0 {
                continue;
            }
            heal.time_until_cooled -= delta.seconds;
        }
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

pub fn remove_channeling(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Channeling)>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut stunned) in query.iter_mut() {
        stunned.duration -= delta.seconds;
        if stunned.duration <= 0.0 {
            commands.entity(entity).remove::<Channeling>();
        }
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

pub fn tick_slow_poison(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut SlowPoisonDebuff,
        &mut AppliedDamage,
        &Hitpoints,
        &mut BoidParams,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut poison, mut damages, hp, mut boid) in query.iter_mut() {
        poison.remaining_time -= delta.seconds;
        damages.damages.push(DamageInstance {
            damage: hp.max_hp * poison.effect_originator.percent_damage * delta.seconds,
            delay: 0.0,
            damage_type: DamageType::Poison,
        });
        if poison.remaining_time <= 0.0 {
            commands.entity(entity).remove::<SlowPoisonDebuff>();
            boid.max_speed = boid.max_speed / poison.effect_originator.speed_multiplier;
        }
    }
}
