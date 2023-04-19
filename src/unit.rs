use core::fmt;

use bevy_ecs::prelude::*;
use gdnative::prelude::*;

pub mod abilities;
pub mod actions;
pub mod effects;
pub mod projectiles;
use actions::*;

use crate::{
    boid::BoidParams,
    event::{DamageCue, EventQueue},
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

use self::{
    abilities::*,
    effects::{ResolveEffectsBuffer, BuffHolder, DivineShieldBuff},
    projectiles::{ActionProjectileDetails, DamageOverride, OnHitEffectsOverride},
};

#[derive(Component, Debug, Clone, Copy)]
pub struct BlueprintId(pub usize);

#[derive(Debug, Clone)]
pub struct UnitBlueprint {
    pub awareness: f32,
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
        awareness: f32,
        mass: f32,
        movespeed: f32,
        acceleration: f32,
        armor: f32,
        magic_resist: f32,
    ) -> Self {
        Self {
            radius: radius,
            mass: mass,
            awareness: awareness,
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

#[derive(Component)]
pub struct DeathApproaches {
    pub spawn_corpse: bool,
    pub cleanup_corpse_canvas: bool,
    pub cleanup_time: f32,
}

impl DeathApproaches {
    pub fn no_corpse() -> Self {
        Self {
            spawn_corpse: false,
            cleanup_corpse_canvas: true,
            cleanup_time: 0.0,
        }
    }

    pub fn new(spawn_corpse: bool, cleanup_corpse: bool, cleanup_time: f32) -> Self {
        Self {
            spawn_corpse: spawn_corpse,
            cleanup_corpse_canvas: cleanup_corpse,
            cleanup_time: cleanup_time,
        }
    }
}

#[derive(Component, Copy, Clone, Debug)]
pub struct StunOnHitEffect {
    pub duration: f32,
    pub stun_texture: Rid,
}

#[derive(Component, Copy, Clone, Debug)]
pub struct AntihealOnHitEffect {
    pub percent_heal_reduction: f32,
    pub duration: f32,
    pub texture: Rid,
}

#[derive(Component, Copy, Clone, Debug)]
pub struct HealEfficacy(pub f32);

#[derive(Component, Copy, Clone)]
pub struct TeamAlignment {
    pub alignment: TeamValue,
    pub alignment_base: TeamValue,
}

#[derive(Component)]
pub struct Hitpoints {
    pub max_hp: f32,
    pub hp: f32,
}


#[derive(Component, Copy, Clone)]
pub struct BaseMass(pub f32);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DamageType {
    Normal,
    Poison,
    Magic,
    Heal,
}

impl fmt::Display for DamageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DamageType::Normal => write!(f, "normal"),
            DamageType::Poison => write!(f, "poison"),
            DamageType::Magic => write!(f, "magic"),
            DamageType::Heal => write!(f, "heal"),
        }
    }
}

#[derive(Debug, Component, Clone, Copy)]
pub struct DamageInstance {
    pub damage: f32,
    pub delay: f32,
    pub damage_type: DamageType,
    pub originator: Entity,
}

#[derive(Component)]
pub struct AppliedDamage {
    pub damages: Vec<DamageInstance>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SlowPoisoned(pub Entity);

#[derive(Component, Clone, Debug, Copy)]
pub struct MeleeWeapon {
    pub damage: f32,
    pub range: f32,

    pub cooldown_time: f32,
    pub impact_time: f32,
    pub full_swing_time: f32,

    pub time_until_weapon_cooled: f32,
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
    pub base: f32,
}

#[derive(Clone, Copy, Component)]
pub struct MagicArmor {
    pub percent_resist: f32,
    pub base: f32,
}

#[derive(Clone, Copy, Component)]
pub struct Speed {
    pub speed: f32,
    pub base: f32,
}

#[derive(Clone, Copy, Component)]
pub struct Acceleration {
    pub acc: f32,
    pub base: f32,
}

#[derive(Component)]
pub struct AttackEnemyBehavior {}

#[derive(Component)]
pub struct HealAllyBehavior {}

pub fn apply_damages(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut AppliedDamage,
        &mut Hitpoints,
        Option<&Armor>,
        Option<&MagicArmor>,
        Option<&HealEfficacy>,
        Option<&BuffHolder>,
    )>,
    data_query: Query<(&TeamAlignment, &BlueprintId)>,
    pos_query: Query<&Position>,
    divine_shield_query: Query<&DivineShieldBuff>,
    delta: Res<DeltaPhysics>,
    mut event_queue: ResMut<EventQueue>,
) {
    for (
        entity,
        mut damages,
        mut hitpoints,
        armor_option,
        magic_armor_option,
        heal_efficacy_option,
        buff_holder_option,
    ) in query.iter_mut()
    {
        let mut divine_shield = false;
        if let Some(buff_holder) = buff_holder_option {
            for buff_ent in buff_holder.set.iter() {
                if let Ok(_divinity) = divine_shield_query.get(*buff_ent) {
                    divine_shield = true;
                }
            }
        }

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
                } else if damage.damage_type == DamageType::Heal {
                    if let Some(heal_efficacy) = heal_efficacy_option {
                        damage.damage *= heal_efficacy.0;
                    }
                }

                if (hitpoints.hp - damage.damage) > hitpoints.max_hp {
                    damage.damage = hitpoints.hp - hitpoints.max_hp;
                }
                if (hitpoints.hp - damage.damage) < 0.0 {
                    damage.damage = hitpoints.hp;
                }

                // Event cue hook
                if let Ok(pos) = pos_query.get(entity) {
                    if let Ok((alignment_target, id_target)) = data_query.get(entity) {
                        if let Ok((alignment_originator, id_originator)) =
                            data_query.get(damage.originator)
                        {
                            let cue = crate::event::EventCue::Damage(DamageCue {
                                damage: damage.damage,
                                damage_type: damage.damage_type.to_string(),
                                location: pos.pos,
                                attacker: crate::event::EventEntityData {
                                    ent: damage.originator,
                                    blueprint: *id_originator,
                                    team: *alignment_originator,
                                },
                                receiver: crate::event::EventEntityData {
                                    ent: entity,
                                    blueprint: *id_target,
                                    team: *alignment_target,
                                },
                            });
                            event_queue.0.push(cue);
                        }
                    }
                }

                if divine_shield {
                    hitpoints.hp = hitpoints.hp.max(hitpoints.hp - damage.damage);
                } else {
                    hitpoints.hp = hitpoints.max_hp.min(hitpoints.hp - damage.damage);
                }
                damages.damages.remove(i);
            } else {
                i += 1;
            }
        }
        if hitpoints.hp <= 0.0 {
            commands
                .entity(entity)
                .insert(DeathApproaches {
                    spawn_corpse: true,
                    cleanup_corpse_canvas: false,
                    cleanup_time: -1.0,
                })
                .remove::<Hitpoints>();
        }
    }
}

pub fn resolve_death(
    mut commands: Commands,
    query: Query<(
        Entity,
        &DeathApproaches,
        Option<&Position>,
        Option<&crate::graphics::Renderable>,
        Option<&crate::graphics::animation::AnimatedSprite>,
        Option<&crate::graphics::ScaleSprite>,
        Option<&OnDeathEffects>,
    )>,
    mut heal_baneling_query: Query<(Entity, &TeamAlignment)>,
    mut damage_query: Query<&mut AppliedDamage>,
) {
    for (
        ent,
        death,
        position_option,
        render_option,
        animated_sprite_option,
        scale_option,
        death_fx_option,
    ) in query.iter()
    {
        if let Some(position) = position_option {
            if let Some(death_fx) = death_fx_option {
                for effect in death_fx.vec.iter() {
                    match effect {
                        effects::DeathEffect::SplashDamage { damage, radius, damage_type } => {
                            commands
                                .spawn()
                                .insert(projectiles::Projectile {
                                    target: ent,
                                    target_pos: position.pos,
                                    origin_action: ent,
                                })
                                .insert(ActionProjectileDetails {
                                    projectile_speed: 0.0,
                                    projectile_scale: 1.0,
                                    projectile_texture: Rid::new(),
                                    contact_distance: 1.0,
                                })
                                .insert(projectiles::DamageOverride { damage: *damage, damage_type: *damage_type })
                                .insert(Position { pos: position.pos })
                                .insert(Velocity { v: Vector2::ZERO })
                                .insert(projectiles::Splash { radius: *radius });
                        }
                        effects::DeathEffect::HealTarget { amount, target } => {
                            if let Ok(mut damage) = damage_query.get_mut(*target) {
                                let heal = DamageInstance {
                                    damage: -*amount,
                                    delay: 0.0,
                                    damage_type: DamageType::Heal,
                                    originator: *target,
                                };
                                damage.damages.push(heal);
                            }
                        }
                        effects::DeathEffect::HealAllies { damage, alignment } => {
                            for (target_ent, test_alignment) in heal_baneling_query.iter_mut() {
                                if test_alignment.alignment == alignment.alignment {
                                    if let Ok(mut damage_buff) = damage_query.get_mut(target_ent) {
                                        let heal = DamageInstance {
                                            damage: -*damage,
                                            delay: 0.0,
                                            damage_type: DamageType::Heal,
                                            originator: ent,
                                        };
                                        damage_buff.damages.push(heal)
                                    }
                                }
                            }
                        }
                        effects::DeathEffect::PoisonSplash {
                            radius,
                            percent_damage,
                            movement_debuff,
                            duration,
                            texture,
                        } => {
                            commands
                                .spawn()
                                .insert(projectiles::Projectile {
                                    target: ent,
                                    target_pos: position.pos,
                                    origin_action: ent,
                                })
                                .insert(ActionProjectileDetails {
                                    projectile_speed: 0.0,
                                    projectile_scale: 1.0,
                                    projectile_texture: Rid::new(),
                                    contact_distance: 12.0,
                                })
                                .insert(OnHitEffectsOverride {
                                    vec: vec![effects::Effect::PoisonEffect {
                                        poison: SlowPoisonAttack {
                                            duration: *duration,
                                            percent_damage: *percent_damage,
                                            movement_debuff: *movement_debuff,
                                            poison_texture: *texture,
                                        },
                                        originator: ent,
                                    }],
                                })
                                .insert(Position { pos: position.pos })
                                .insert(Velocity { v: Vector2::ZERO })
                                .insert(projectiles::Splash { radius: *radius });
                        }
                    }
                }
            }
            if death.spawn_corpse {
                if let Some(sprite) = animated_sprite_option {
                    let mut animated_sprite = crate::graphics::animation::AnimatedSprite::default();
                    animated_sprite.texture = sprite.texture;

                    // Negative timeout will be ignored and discarded by timeout system
                    let mut timeout = death.cleanup_time;
                    if !death.cleanup_corpse_canvas {
                        timeout = -1.0
                    }

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
        }

        commands.entity(ent).despawn();
        if let Some(renderable) = render_option {
            commands
                .spawn()
                .insert(CleanupCanvasItem(renderable.canvas_item_rid));
        }
    }
}

pub fn spawn_projectile(commands: &mut Commands, origin_pos: Vector2, splash_radius: f32) {}
