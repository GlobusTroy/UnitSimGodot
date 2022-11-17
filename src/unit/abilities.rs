use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    boid::BoidParams,
    graphics::{
        animation::{AnimatedSprite, PlayAnimationDirective},
        FlippableSprite, NewCanvasItemDirective, ScaleSprite,
    },
    physics::{DeltaPhysics, Position},
    util::ExpirationTimer,
};

use super::{Casting, Channeling, Hitpoints, SlowPoisoned, Stunned};

#[derive(Debug, Clone)]
pub enum UnitAbility {
    Cleanse(CleanseAbility),
    SlowPoison(SlowPoisonAttack),
    Heal(HealAbility),
    MagicMissile(MagicMissileAbility),
}

#[derive(Component, Debug, Clone, Copy)]
pub struct CleanseAbility {
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub effect_texture: Rid,

    pub time_until_cleanse_cooled: f32,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct HealAbility {
    pub heal_amount: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub effect_texture: Rid,

    pub time_until_cooled: f32,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct MagicMissileAbility {
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub effect_texture: Rid,

    pub time_until_cooled: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SlowPoisonAttack {
    pub duration: f32,
    pub percent_damage: f32,
    pub speed_multiplier: f32,
    pub poison_texture: Rid,
}
