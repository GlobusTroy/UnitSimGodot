use bevy_ecs::prelude::*;
use gdnative::prelude::*;

pub mod abilities;
pub mod actions;
pub mod effects;
pub mod projectiles;
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
    Heal,
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
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (
        entity,
        mut damages,
        mut hitpoints,
        armor_option,
        magic_armor_option,
        heal_efficacy_option,
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
                } else if damage.damage_type == DamageType::Heal {
                    if let Some(heal_efficacy) = heal_efficacy_option {
                        damage.damage *= heal_efficacy.0;
                    }
                }
                hitpoints.hp = hitpoints.max_hp.min(hitpoints.hp - damage.damage);
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
    )>,
) {
    for (ent, death, position_option, render_option, animated_sprite_option, scale_option) in
        query.iter()
    {
        if let Some(position) = position_option {
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
