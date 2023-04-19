use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{
    graphics::{animation::AnimatedSprite, CleanupCanvasItem, ModulateSprite, ScaleSprite},
    util::MirrorTargetPosition,
};

use super::{
    abilities::{DamageBuffAbility, FortifyAbility, OverdriveAbility},
    actions::{Cooldown, OnDeathEffects, OnHitEffects, TargetEntity, UnitActions},
    Acceleration, AntihealOnHitEffect, AppliedDamage, Armor, DamageInstance, DeathApproaches,
    MagicArmor, StunOnHitEffect, TeamAlignment, TeamValue, DamageType,
};

#[derive(Copy, Clone)]
pub enum Effect {
    DamageEffect(DamageInstance),
    PoisonEffect {
        poison: super::abilities::SlowPoisonAttack,
        originator: Entity,
    },
    ArmorReductionEffect(super::abilities::ArmorReductionAttack),
    StunEffect(StunOnHitEffect),
    Hypnosis{alignment: super::TeamValue, duration: f32},
    ShredArmorEffect(super::abilities::ShredArmorAttack),
    CleanseEffect,
    DivineShieldEffect{duration: f32},
    HealEffect {
        amount: f32,
        originator: Entity,
    },
    HealOverTimeEffect {
        amount_per_second: f32,
        duration: f32,
        originator: Entity,
    },
    SuicideEffect {
        originator: Entity,
    },
    AttackSpeedBuff{percent_cooldown_reduction: f32},
    ApplyStatBuffEffect(StatBuff, f32),
    OverdriveEffect(OverdriveAbility),
    DamageBuffEffect(DamageBuffAbility),
    ConfusionEffect(super::abilities::ConfusionAttack),
    TeleportBehindTargetEffect(Entity),
    HealOnDeathEffect {
        amount: f32,
        target: Entity,
    },
    AntiHeal(AntihealOnHitEffect),
    Visual(SpawnVisualEffect),
}

#[derive(Copy, Clone)]
pub enum DeathEffect {
    SplashDamage {
        damage: f32,
        radius: f32,
        damage_type: DamageType
    },
    HealAllies {
        damage: f32,
        alignment: TeamAlignment,
    },
    HealTarget {
        amount: f32,
        target: Entity,
    },
    PoisonSplash {
        radius: f32,
        percent_damage: f32,
        movement_debuff: f32,
        duration: f32,
        texture: Rid,
    },
}

#[derive(Component)]
pub struct BuffTimer(pub f32);

#[derive(Component)]
pub struct BuffType {
    pub is_debuff: bool,
}

#[derive(Component)]
pub struct PercentDamageOverTime {
    pub damage_percent: f32,
    pub damage_type: super::DamageType,
    pub originator: Entity,
}

#[derive(Component)]
pub struct TeleportToPointEffect(pub Vector2);

#[derive(Component)]
pub struct ResolveEffectsBuffer {
    pub vec: Vec<Effect>,
}

#[derive(Component, Clone, Debug)]
pub struct BuffHolder {
    pub set: std::collections::HashSet<Entity>,
}

#[derive(Component, Clone, Debug)]
pub struct HealingPerSecond {
    pub amount: f32,
    pub originator: Entity,
}

#[derive(Component, Clone, Debug)]
pub struct FlatDamageBuff(pub f32);

#[derive(Component, Clone, Debug)]
pub struct PercentCooldownReduction(pub f32);

#[derive(Component, Clone, Debug)]
pub struct PercentHealReduction(pub f32);

#[derive(Component, Clone, Debug)]
pub struct SetAcceleration(pub f32);

#[derive(Component, Clone, Debug)]
pub struct SetArmor(pub f32);

#[derive(Component, Clone, Debug)]
pub struct SetAlignment(pub TeamValue);

#[derive(Component, Clone, Debug)]
pub struct StunnedBuff {}

#[derive(Component, Clone, Debug)]
pub struct DivineShieldBuff{}

#[derive(Clone, Copy, Debug)]
pub struct SpawnVisualEffect {
    pub texture: Rid,
    pub duration: f32,
}

#[derive(Component, Copy, Clone, Debug, Default)]
pub struct StatBuff {
    pub mass_buff: f32,
    pub armor_buff: f32,
    pub magic_armor_buff: f32,
    pub speed_buff: f32,
    pub acceleration_buff: f32,
    pub heal_efficacy_mult_buff: f32,
}

pub fn resolve_effects(
    mut commands: Commands,
    mut query: Query<(Entity, &mut super::effects::ResolveEffectsBuffer)>,
    mut on_death: Query<&mut OnDeathEffects>,
    mut damage_query: Query<&mut crate::unit::AppliedDamage>,
    mut buff_holder_query: Query<&mut BuffHolder>,
    mut actions_query: Query<&mut UnitActions>,
    action_query: Query<&Cooldown>,
    mut action_damage_query: Query<&mut OnHitEffects>,
    buff_query: Query<&BuffType>,
    pos_rad_query: Query<(&crate::physics::Position, &crate::physics::Radius)>,
) {
    for (ent, mut buffer) in query.iter_mut() {
        for effect in buffer.vec.iter() {
            match effect {
                // POISON
                Effect::PoisonEffect { poison, originator } => {
                    let poison_buff = spawn_poison_buff(&mut commands, poison, ent, *originator);
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        buff_holder.set.insert(poison_buff);
                    }
                }

                // STUN
                Effect::StunEffect(stun) => {
                    let stun_buff = spawn_stun_buff(&mut commands, stun, ent);
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        buff_holder.set.insert(stun_buff);
                    }
                }

                // Heal On Death
                Effect::HealOnDeathEffect { amount, target } => {
                    if let Ok(mut on_death) = on_death.get_mut(ent) {
                        let heal = DeathEffect::HealTarget {
                            amount: *amount,
                            target: *target,
                        };
                        on_death.vec.push(heal);
                    }
                }

                // SUICIDE / BANELING
                Effect::SuicideEffect { originator } => {
                    commands.entity(*originator).insert(DeathApproaches {
                        spawn_corpse: true,
                        cleanup_corpse_canvas: true,
                        cleanup_time: 3.0,
                    });
                }

                // DAMAGE
                Effect::DamageEffect(damage_instance) => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(*damage_instance);
                    }
                }

                // HEAL
                Effect::HealEffect { amount, originator } => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(DamageInstance {
                            damage: -amount,
                            delay: 0.0,
                            damage_type: super::DamageType::Heal,
                            originator: *originator,
                        });
                    }
                }

                // HEAL OVER TIME
                Effect::HealOverTimeEffect {
                    amount_per_second,
                    duration,
                    originator,
                } => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        if let Ok(actions) = actions_query.get(ent) {
                            for action in actions.vec.iter() {
                                if let Ok(_cooldown) = action_query.get(*action) {
                                    let buff = commands
                                        .spawn()
                                        .insert(BuffType { is_debuff: false })
                                        .insert(BuffTimer(*duration))
                                        .insert(TargetEntity { entity: ent })
                                        .insert(HealingPerSecond {
                                            amount: *amount_per_second,
                                            originator: *originator,
                                        })
                                        .id();

                                    buff_holder.set.insert(buff);
                                }
                            }
                        }
                    }
                }

                // CLEANSE
                Effect::CleanseEffect => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let mut cleansed = Vec::<Entity>::new();

                        // Get each debuff into cleansed
                        for buff_ent in buff_holder.set.iter() {
                            if let Ok(buff) = buff_query.get(*buff_ent) {
                                if buff.is_debuff {
                                    cleansed.push(*buff_ent);
                                }
                            }
                        }

                        // Remove all cleansed debuffs
                        for buff_ent in cleansed.iter() {
                            buff_holder.set.remove(buff_ent);
                            commands
                                .entity(*buff_ent)
                                .insert(DeathApproaches::no_corpse());
                        }
                    }
                }

                // Statbuff
                Effect::ApplyStatBuffEffect(buff, duration) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = commands
                            .spawn()
                            .insert(BuffType { is_debuff: false })
                            .insert(BuffTimer(*duration))
                            .insert(TargetEntity { entity: ent })
                            .insert(*buff)
                            .id();

                        buff_holder.set.insert(buff);
                    }
                }

                // Statbuff
                Effect::DivineShieldEffect { duration } => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = commands
                            .spawn()
                            .insert(BuffType { is_debuff: false })
                            .insert(BuffTimer(*duration))
                            .insert(TargetEntity { entity: ent })
                            .insert(DivineShieldBuff{})
                            .id();

                        buff_holder.set.insert(buff);
                    }
                }

                // Overdrive
                Effect::OverdriveEffect(overdrive) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        if let Ok(actions) = actions_query.get(ent) {
                            for action in actions.vec.iter() {
                                if let Ok(_cooldown) = action_query.get(*action) {
                                    let buff =
                                        spawn_overdrive_buff(&mut commands, overdrive, action);

                                    buff_holder.set.insert(buff);
                                }
                            }
                            let buff = spawn_visual_buff(
                                &mut commands,
                                overdrive.duration,
                                overdrive.effect_texture,
                                ent,
                                ModulateSprite {
                                    r: 0.8,
                                    b: 1.0,
                                    g: 0.2,
                                },
                            );

                            buff_holder.set.insert(buff);
                        }
                    }
                }

                // Attack Speed Buff 
                Effect::AttackSpeedBuff{percent_cooldown_reduction} => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        if let Ok(actions) = actions_query.get(ent) {
                            // First action SHOULD be basic attack
                            if let Some(action) = actions.vec.get(0) {
                                if let Ok(_cooldown) = action_query.get(*action) {
                                    let buff = commands
                                            .spawn()
                                            .insert(BuffType { is_debuff: false })
                                            .insert(BuffTimer(200.0))
                                            .insert(PercentCooldownReduction(*percent_cooldown_reduction))
                                            .insert(TargetEntity { entity: *action })
                                            .id();
                                    buff_holder.set.insert(buff);
                                }
                            }
                        }
                    }
                }

                // DamageBuff
                Effect::DamageBuffEffect(overdrive) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        if let Ok(mut actions) = actions_query.get_mut(ent) {
                            for action in actions.vec.iter_mut() {
                                if let Ok(mut on_hit_effects) = action_damage_query.get_mut(*action)
                                {
                                    let mut new_instance = None;
                                    let mut index = 0;
                                    for (i, effect) in on_hit_effects.vec.iter().enumerate() {
                                        // If damage effect is applied, apply buff to action
                                        if let Effect::DamageEffect(instance) = effect {
                                            index = i;
                                            let mut curr_instance = instance.clone();
                                            curr_instance.damage *= 1. + overdrive.damage;
                                            new_instance = Some(curr_instance);
                                        }
                                        break;
                                    }
                                    if let Some(instance) = new_instance {
                                        on_hit_effects.vec.remove(index);
                                        on_hit_effects.vec.push(Effect::DamageEffect(instance));
                                    }
                                }
                            }
                            let buff = spawn_visual_buff(
                                &mut commands,
                                overdrive.duration,
                                overdrive.texture,
                                ent,
                                ModulateSprite {
                                    r: 1.0,
                                    b: 0.2,
                                    g: 0.8,
                                },
                            );

                            buff_holder.set.insert(buff);
                        }
                    }
                }

                // Confusion
                Effect::ConfusionEffect(confusion) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_confusion_buff(&mut commands, confusion, ent);

                        buff_holder.set.insert(buff);
                    }
                }

                // Armor Reduction Effect
                Effect::ArmorReductionEffect(reduction) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_armor_debuff(&mut commands, reduction, ent);

                        buff_holder.set.insert(buff);
                    }
                }

                // Shred Armor
                Effect::ShredArmorEffect(shred_armor) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_armor_shred_debuff(&mut commands, shred_armor, ent);

                        buff_holder.set.insert(buff);
                    }
                }

                // Antiheal
                Effect::AntiHeal(antiheal) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_antiheal_buff(&mut commands, antiheal, ent);

                        buff_holder.set.insert(buff);
                    }
                }

                // Hypnosis 
                Effect::Hypnosis { alignment, duration } => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_hypnosis_buff(&mut commands, *duration, *alignment, ent);

                        buff_holder.set.insert(buff);
                    }
                }

                // TeleportBehindTargetEffect
                Effect::TeleportBehindTargetEffect(teleported) => {
                    if let Ok((t_pos, t_rad)) = pos_rad_query.get(ent) {
                        if let Ok((teleport_pos, teleport_rad)) = pos_rad_query.get(*teleported) {
                            let offset = teleport_pos.pos.direction_to(t_pos.pos)
                                * (t_rad.r + teleport_rad.r);
                            let result_teleport_pos = offset + t_pos.pos;
                            if let Ok(mut buff_holder) = buff_holder_query.get_mut(*teleported) {
                                let buff = commands
                                    .spawn()
                                    .insert(TeleportToPointEffect(result_teleport_pos))
                                    .insert(TargetEntity {
                                        entity: *teleported,
                                    })
                                    .id();
                                buff_holder.set.insert(buff);
                            }
                        }
                    }
                }

                // Visual
                Effect::Visual(visual) => {
                    if let Ok(mut buff_holder) = buff_holder_query.get_mut(ent) {
                        let buff = spawn_visual_buff(
                            &mut commands,
                            visual.duration,
                            visual.texture,
                            ent,
                            ModulateSprite {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                            },
                        );
                        buff_holder.set.insert(buff);
                    }
                }

                _ => (),
            }
        }

        buffer.vec.clear();
    }
}

fn spawn_antiheal_buff(
    commands: &mut Commands,
    antiheal: &super::AntihealOnHitEffect,
    ent: Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(BuffTimer(antiheal.duration))
        .insert(StatBuff {
            mass_buff: 0.,
            armor_buff: 0.,
            magic_armor_buff: 0.0,
            speed_buff: 0.0,
            acceleration_buff: 0.0,
            heal_efficacy_mult_buff: antiheal.percent_heal_reduction,
        })
        .insert(TargetEntity { entity: ent })
        .insert(MirrorTargetPosition {})
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(crate::graphics::AlphaSprite(0.35))
        .insert(crate::graphics::ModulateSprite {
            r: 1.0,
            g: 0.2,
            b: 0.2,
        })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(antiheal.texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    buff
}

fn spawn_armor_debuff(
    commands: &mut Commands,
    debuff: &super::abilities::ArmorReductionAttack,
    ent: Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(BuffTimer(debuff.duration))
        .insert(TargetEntity { entity: ent })
        .insert(MirrorTargetPosition {})
        .insert(StatBuff {
            mass_buff: 0.,
            armor_buff: -debuff.armor_reduction,
            magic_armor_buff: -debuff.magic_armor_reduction,
            speed_buff: 0.0,
            acceleration_buff: 0.0,
            heal_efficacy_mult_buff: 0.0,
        })
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(crate::graphics::AlphaSprite(0.35))
        .insert(crate::graphics::ModulateSprite {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(debuff.texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    buff
}

fn spawn_armor_shred_debuff(
    commands: &mut Commands,
    debuff: &super::abilities::ShredArmorAttack,
    ent: Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(BuffTimer(debuff.duration))
        .insert(TargetEntity { entity: ent })
        .insert(MirrorTargetPosition {})
        .insert(SetArmor(0.0))
        .insert(StatBuff {
            mass_buff: 0.,
            armor_buff: 0.0,
            magic_armor_buff: 0.0,
            speed_buff: 0.0,
            acceleration_buff: 0.0,
            heal_efficacy_mult_buff: 0.5,
        })
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(crate::graphics::AlphaSprite(0.35))
        .insert(crate::graphics::ModulateSprite {
            r: 0.5,
            g: 0.0,
            b: 0.0,
        })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(debuff.texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    buff
}

fn spawn_confusion_buff(
    commands: &mut Commands,
    confusion: &super::abilities::ConfusionAttack,
    ent: Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(SetAcceleration(confusion.set_acceleration))
        .insert(BuffTimer(confusion.duration))
        .insert(TargetEntity { entity: ent })
        .insert(MirrorTargetPosition {})
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(crate::graphics::AlphaSprite(0.35))
        .insert(crate::graphics::ModulateSprite {
            r: 1.0,
            g: 0.2,
            b: 0.2,
        })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(confusion.texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    buff
}

fn spawn_visual_buff(
    commands: &mut Commands,
    duration: f32,
    texture: Rid,
    ent: Entity,
    modulate: ModulateSprite,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: false })
        .insert(BuffTimer(duration))
        .insert(MirrorTargetPosition {})
        .insert(TargetEntity { entity: ent })
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(modulate)
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    buff
}

fn spawn_overdrive_buff(
    commands: &mut Commands,
    overdrive: &OverdriveAbility,
    action: &Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: false })
        .insert(BuffTimer(overdrive.duration))
        .insert(PercentCooldownReduction(overdrive.percent_cooldown_speedup))
        .insert(TargetEntity { entity: *action })
        .id();
    buff
}

fn spawn_hypnosis_buff(
    commands: &mut Commands,
    duration: f32,
    alignment: super::TeamValue,
    ent: Entity,
) -> Entity {
    let buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(BuffTimer(duration))
        .insert(SetAlignment(alignment))
        .insert(TargetEntity { entity: ent })
        .id();
    buff
}

fn spawn_poison_buff(
    commands: &mut Commands,
    poison: &super::abilities::SlowPoisonAttack,
    target_ent: Entity,
    originator_ent: Entity,
) -> Entity {
    let poison_buff = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(PercentDamageOverTime {
            damage_percent: poison.percent_damage,
            damage_type: super::DamageType::Poison,
            originator: originator_ent,
        })
        .insert(StatBuff {
            mass_buff: 0.,
            armor_buff: 0.0,
            magic_armor_buff: 0.0,
            speed_buff: -poison.movement_debuff,
            acceleration_buff: -poison.movement_debuff,
            heal_efficacy_mult_buff: 0.0,
        })
        .insert(BuffTimer(poison.duration))
        .insert(TargetEntity { entity: target_ent })
        .insert(MirrorTargetPosition {})
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(ScaleSprite(Vector2 { x: 0.75, y: 0.75 }))
        .insert(crate::graphics::AlphaSprite(0.35))
        .insert(crate::graphics::ModulateSprite {
            r: 0.6,
            g: 0.25,
            b: 1.,
        })
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(poison.poison_texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    poison_buff
}

fn spawn_stun_buff(commands: &mut Commands, stun: &StunOnHitEffect, target_ent: Entity) -> Entity {
    let stun = commands
        .spawn()
        .insert(BuffType { is_debuff: true })
        .insert(BuffTimer(stun.duration))
        .insert(TargetEntity { entity: target_ent })
        .insert(MirrorTargetPosition {})
        .insert(StunnedBuff {})
        .insert(crate::physics::Position { pos: Vector2::ZERO })
        .insert(crate::physics::Velocity { v: Vector2::ZERO })
        .insert(crate::graphics::ModulateSprite {
            r: 0.25,
            g: 1.0,
            b: 1.0,
        })
        .insert(crate::graphics::AlphaSprite(0.75))
        .insert(crate::graphics::NewCanvasItemDirective {})
        .insert(AnimatedSprite::new(stun.stun_texture))
        .insert(crate::graphics::animation::PlayAnimationDirective {
            animation_name: "fly".to_string(),
            is_one_shot: false,
        })
        .id();
    stun
}

pub fn set_stats_directly(
    query: Query<(Option<&SetAcceleration>, Option<&SetArmor>, Option<&SetAlignment>, &TargetEntity)>,
    mut acceleration_query: Query<&mut Acceleration>,
    mut armor_query: Query<&mut Armor>,
    mut alignment_query: Query<&mut TeamAlignment>,
) {
    for (set_val_acc_opt, set_val_armor_opt, set_val_alignment_opt, target) in query.iter() {
        if let Some(set_acc) = set_val_acc_opt {
            if let Ok(mut acceleration) = acceleration_query.get_mut(target.entity) {
                acceleration.acc = set_acc.0;
            }
        }
        if let Some(set_armor) = set_val_armor_opt {
            if let Ok(mut armor) = armor_query.get_mut(target.entity) {
                armor.armor = set_armor.0;
            }
        }
        if let Some(set_alignment) = set_val_alignment_opt {
            if let Ok(mut alignment) = alignment_query.get_mut(target.entity) {
                alignment.alignment = set_alignment.0;
            }
        }
    }
}

pub fn apply_teleport(
    mut commands: Commands,
    query: Query<(Entity, &TeleportToPointEffect, &TargetEntity)>,
    mut teleport_query: Query<&mut crate::physics::Position>,
) {
    for (ent, teleport, target) in query.iter() {
        if let Ok(mut position) = teleport_query.get_mut(target.entity) {
            position.pos = teleport.0;
            commands.entity(ent).insert(BuffTimer(0.0));
        }
    }
}


pub fn reset_alignment(mut query: Query<&mut TeamAlignment>) {
    for mut alignment in query.iter_mut() {
        alignment.alignment = alignment.alignment_base;
    } 
}

pub fn apply_stat_buffs(
    mut query: Query<(
        &BuffHolder,
        &crate::BaseMass,
        &mut crate::physics::Mass,
        &mut super::Armor,
        &mut MagicArmor,
        &mut super::Speed,
        &mut super::Acceleration,
        &mut super::HealEfficacy,
    )>,
    buff_query: Query<&StatBuff>,
) {
    for (buff_holder, base_mass, mut mass, mut armor, mut magic_armor, mut speed, mut acceleration, mut heal_efficacy) in
        query.iter_mut()
    {
        mass.0 = base_mass.0;
        armor.armor = armor.base;
        magic_armor.percent_resist = magic_armor.base;
        speed.speed = speed.base;
        acceleration.acc = acceleration.base;
        heal_efficacy.0 = 1.0;

        for buff_entity in buff_holder.set.iter() {
            if let Ok(buff) = buff_query.get(*buff_entity) {
                mass.0 += buff.mass_buff;
                armor.armor += buff.armor_buff;
                magic_armor.percent_resist += buff.magic_armor_buff;
                speed.speed = (speed.speed + buff.speed_buff).max(1.0);
                acceleration.acc = (acceleration.acc + buff.acceleration_buff).max(1.0);
                heal_efficacy.0 *= 1. - buff.heal_efficacy_mult_buff;
            }
        }
    }
}

pub fn percent_damage_over_time(
    buff_query: Query<(&PercentDamageOverTime, &TargetEntity)>,
    mut target_query: Query<(&super::Hitpoints, &mut ResolveEffectsBuffer)>,
    delta: Res<crate::physics::DeltaPhysics>,
) {
    for (damage, ent_target) in buff_query.iter() {
        if let Ok((hp, mut target)) = target_query.get_mut(ent_target.entity) {
            target.vec.push(Effect::DamageEffect(DamageInstance {
                damage: hp.max_hp * damage.damage_percent * delta.seconds,
                delay: 0.0,
                damage_type: damage.damage_type,
                originator: damage.originator,
            }))
        }
    }
}

pub fn heal_over_time(
    buff_query: Query<(&HealingPerSecond, &TargetEntity)>,
    mut target_query: Query<&mut ResolveEffectsBuffer>,
    delta: Res<crate::physics::DeltaPhysics>,
) {
    for (healing, ent_target) in buff_query.iter() {
        if let Ok(mut target) = target_query.get_mut(ent_target.entity) {
            target.vec.push(Effect::DamageEffect(DamageInstance {
                damage: -healing.amount * delta.seconds,
                delay: 0.0,
                damage_type: super::DamageType::Heal,
                originator: healing.originator,
            }))
        }
    }
}

pub fn percent_cooldown_speedup(
    buff_query: Query<(&PercentCooldownReduction, &TargetEntity)>,
    mut cooldown_query: Query<&mut Cooldown>,
    delta: Res<crate::physics::DeltaPhysics>,
) {
    for (reduction, ent_target) in buff_query.iter() {
        if let Ok(mut cooldown) = cooldown_query.get_mut(ent_target.entity) {
            cooldown.0 -= delta.seconds * reduction.0;
        }
    }
}

pub fn apply_stun_buff(
    mut commands: Commands,
    buff_query: Query<(Entity, &StunnedBuff, &TargetEntity)>,
    mut target_query: Query<&ResolveEffectsBuffer>,
) {
    for (ent, _stun, ent_target) in buff_query.iter() {
        // Safety guardrail against despawned entity
        if let Ok(_) = target_query.get_mut(ent_target.entity) {
            commands
                .entity(ent_target.entity)
                .insert(super::actions::PerformingActionState { action: ent });
        }
    }
}

pub fn buff_timer(
    mut commands: Commands,
    mut holder_query: Query<&mut BuffHolder>,
    mut buff_query: Query<(
        Entity,
        &mut BuffTimer,
        &TargetEntity,
        Option<&crate::graphics::Renderable>,
    )>,
    delta: Res<crate::physics::DeltaPhysics>,
) {
    for (ent, mut timer, target, render_option) in buff_query.iter_mut() {
        timer.0 -= delta.seconds;

        let mut should_cleanup = timer.0 <= 0.0;
        // Remove buff if target is removed
        if let Err(bevy_ecs::query::QueryEntityError::NoSuchEntity(_)) =
            holder_query.get(target.entity)
        {
            should_cleanup = true;
        }

        if should_cleanup {
            if let Some(renderable) = render_option {
                commands
                    .spawn()
                    .insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
            commands.entity(ent).insert(DeathApproaches {
                spawn_corpse: false,
                cleanup_corpse_canvas: true,
                cleanup_time: 0.0,
            });
            if let Ok(mut buff_holder) = holder_query.get_mut(target.entity) {
                buff_holder.set.remove(&ent);
            }
        }
    }
}
