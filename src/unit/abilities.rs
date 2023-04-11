use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use super::AntihealOnHitEffect;

#[derive(Debug, Clone)]
pub enum UnitAbility {
    Cleanse(CleanseAbility),
    SlowPoison(SlowPoisonAttack),
    Stun(super::StunOnHitEffect),
    Heal(HealAbility),
    MagicMissile(MagicMissileAbility),
    BubbleBomb(BubbleBombAbility),
    Whirlwind(WhirlwindAbility),
    Overdrive(OverdriveAbility),
    Confusion(ConfusionAttack),
    Backstab(BackstabAbility),
    DamageBuff(DamageBuffAbility),
    AntiHeal(AntihealOnHitEffect),
    ArmorReduction(ArmorReductionAttack),
    Fortify(FortifyAbility),
    BuffResistance(BuffResistanceAbility),
    BanelingAttack { damage: f32, radius: f32 },
    ExecutionAttack { heal_amount: f32 },
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

#[derive(Debug, Component, Clone, Copy)]
pub struct BubbleBombAbility {
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub radius: f32,
    pub stun_duration: f32,
    pub effect_texture: Rid,

    pub time_until_cooled: f32,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct WhirlwindAbility {
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct OverdriveAbility {
    pub percent_cooldown_speedup: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub duration: f32,
    pub effect_texture: Rid,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct FortifyAbility {
    pub heal_immediate: f32,
    pub heal_over_time: f32,
    pub armor_amount: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub duration: f32,
    pub effect_texture: Rid,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct BuffResistanceAbility {
    pub magic_armor_amount: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub duration: f32,
    pub effect_texture: Rid,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct BackstabAbility {
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub texture: Rid,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct DamageBuffAbility {
    pub damage: f32,
    pub range: f32,
    pub cooldown: f32,
    pub duration: f32,
    pub swing_time: f32,
    pub impact_time: f32,
    pub texture: Rid,
}

#[derive(Debug, Component, Clone, Copy)]
pub struct ConfusionAttack {
    pub set_acceleration: f32,
    pub duration: f32,
    pub texture: Rid,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SlowPoisonAttack {
    pub duration: f32,
    pub percent_damage: f32,
    pub movement_debuff: f32,
    pub poison_texture: Rid,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct ArmorReductionAttack {
    pub duration: f32,
    pub armor_reduction: f32,
    pub magic_armor_reduction: f32,
    pub texture: Rid,
}
