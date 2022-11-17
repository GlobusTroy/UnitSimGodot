use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{util::MirrorTargetPosition, graphics::{animation::AnimatedSprite, CleanupCanvasItem, ScaleSprite}};

use super::{MagicArmor, StunOnHitEffect, actions::TargetEntity, AppliedDamage, DamageInstance, DeathApproaches};

#[derive(Copy, Clone)]
pub enum Effect {
    DamageEffect(DamageInstance),
    PoisonEffect(super::abilities::SlowPoisonAttack),
    StunEffect(StunOnHitEffect),
    CleanseEffect,
    HealEffect(f32),
}
#[derive(Component)]
pub struct BuffTimer(pub f32);

#[derive(Component)]
pub struct BuffType {
    pub is_debuff: bool,
}

#[derive(Component)]
pub struct PercentDamageOverTime{
    pub damage_percent: f32,
    pub damage_type: super::DamageType,
}

#[derive(Component)]
pub struct ResolveEffectsBuffer {
    pub vec: Vec<Effect>,
}

#[derive(Component, Clone, Debug)]
pub struct BuffHolder {
    pub set: std::collections::HashSet<Entity>,
}

pub struct ActionBuff {
    pub cooldown_buff: f32,
    pub damage_buff: f32,
    pub range_buff: f32,
}

#[derive(Component, Clone, Debug)]
pub struct StatBuff {
    pub armor_buff: f32,
    pub magic_armor_buff: f32,
    pub speed_buff: f32,
    pub acceleration_buff: f32,
}

pub fn resolve_effects(
    mut commands: Commands,
    mut query: Query<(Entity, &mut super::effects::ResolveEffectsBuffer)>,
    mut damage_query: Query<&mut crate::unit::AppliedDamage>,
    mut buff_query: Query<&mut BuffHolder>,
) {
    for (ent, mut buffer) in query.iter_mut() {
        for effect in buffer.vec.iter() {
            match effect {
                // POISON 
                Effect::PoisonEffect(poison) => {
                    let poison_buff = commands
                        .spawn()
                        .insert(BuffType { is_debuff: true })
                        .insert(PercentDamageOverTime{ damage_percent: poison.percent_damage, damage_type: super::DamageType::Poison })
                        .insert(BuffTimer(poison.duration))
                        .insert(TargetEntity{entity: ent})
                        .insert(MirrorTargetPosition{})
                        .insert(crate::physics::Position{pos: Vector2::ZERO})
                        .insert(crate::physics::Velocity{v: Vector2::ZERO})
                        .insert(ScaleSprite(Vector2{x: 0.75, y: 0.75}))
                        .insert(crate::graphics::AlphaSprite(0.35))
                        .insert(crate::graphics::ModulateSprite{r: 0.6, g: 0.25, b: 1.})
                        .insert(crate::graphics::NewCanvasItemDirective{})
                        .insert(AnimatedSprite::new(poison.poison_texture))
                        .insert(crate::graphics::animation::PlayAnimationDirective{animation_name: "fly".to_string(), is_one_shot: false})
                        .id();
                    commands.entity(poison_buff).insert(StatBuff {
                        armor_buff: 0.0,
                        magic_armor_buff: 0.0,
                        speed_buff: -3.5,
                        acceleration_buff: -5.0,
                    });
                    if let Ok(mut buff_holder) = buff_query.get_mut(ent) {
                        buff_holder.set.insert(poison_buff);
                    }
                }

                // STUN
                Effect::StunEffect(stun) => {
                    commands
                        .entity(ent)
                        .insert(super::Stunned {
                            duration: stun.duration,
                        })
                        .remove::<super::actions::PerformingActionState>();
                }

                // DAMAGE
                Effect::DamageEffect(damage_instance) => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(*damage_instance);
                    }
                }

                // HEAL
                Effect::HealEffect(heal_amount) => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(DamageInstance {
                            damage: -heal_amount,
                            delay: 0.0,
                            damage_type: super::DamageType::Heal,
                        });
                    }
                }
                _ => (),
            }
        }

        buffer.vec.clear();
    }
}

pub fn apply_stat_buffs(
    mut query: Query<(
        &BuffHolder,
        &mut super::Armor,
        &mut MagicArmor,
        &mut super::Speed,
        &mut super::Acceleration,
    )>,
    buff_query: Query<&StatBuff>,
) {
    for (
        buff_holder,
        mut armor,
        mut magic_armor,
        mut speed,
        mut acceleration,
    ) in query.iter_mut()
    {
        armor.armor = armor.base;
        magic_armor.percent_resist = magic_armor.base;
        speed.speed = speed.base;
        acceleration.acc = acceleration.base;

        for buff_entity in buff_holder.set.iter() {
            if let Ok(buff) = buff_query.get(*buff_entity) {
                armor.armor += buff.armor_buff;
                magic_armor.percent_resist += buff.magic_armor_buff;
                speed.speed = (speed.speed + buff.speed_buff).max(1.0);
                acceleration.acc = (acceleration.acc + buff.acceleration_buff).max(1.0);
            }
        }
    }
}

pub fn percent_damage_over_time(buff_query: Query<(&PercentDamageOverTime, &TargetEntity)>, mut target_query: Query<(&super::Hitpoints, &mut ResolveEffectsBuffer)>, delta: Res<crate::physics::DeltaPhysics>) {
    for (damage, ent_target) in buff_query.iter() {
        if let Ok((hp, mut target)) = target_query.get_mut(ent_target.entity) {
            target.vec.push(Effect::DamageEffect(DamageInstance{damage: hp.max_hp * damage.damage_percent * delta.seconds, delay: 0.0, damage_type: damage.damage_type}))
        } 
    }
}

pub fn buff_timer(mut commands: Commands, mut holder_query: Query<&mut BuffHolder>, mut buff_query: Query<(Entity, &mut BuffTimer, &TargetEntity, Option<&crate::graphics::Renderable>)>, delta: Res<crate::physics::DeltaPhysics>) {
    for (ent, mut timer, target, render_option) in buff_query.iter_mut() {
        timer.0 -= delta.seconds;

        let mut should_cleanup = timer.0 <= 0.0; 
        // Remove buff if target is removed
        if let Err(bevy_ecs::query::QueryEntityError::NoSuchEntity(_)) = holder_query.get(target.entity) {
            should_cleanup = true;
        }

        if should_cleanup  {
            if let Some(renderable) = render_option {
                commands.spawn().insert(CleanupCanvasItem(renderable.canvas_item_rid));
            }
            commands.entity(ent).insert(DeathApproaches{spawn_corpse:false, cleanup_corpse_canvas: true, cleanup_time: 0.0 });
            if let Ok(mut buff_holder) = holder_query.get_mut(target.entity) {
                buff_holder.set.remove(&ent);
            }
        }

    }
}
