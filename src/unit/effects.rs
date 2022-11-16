use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use super::StunOnHitEffect;



#[derive(Copy, Clone)]
pub enum Effect {
    DamageEffect(super::DamageInstance),
    PoisonEffect(super::abilities::SlowPoisonAttack),
    StunEffect(StunOnHitEffect),
}



#[derive(Component)]
pub struct ResolveEffectsBuffer {
    pub vec: Vec<Effect>,
}

#[derive(Component, Clone, Debug)]
pub struct BuffHolder {
    vec: Vec<Entity>
}

pub struct ActionBuff {
    pub cooldown_buff: f32,
    pub damage_buff: f32,
    pub range_buff: f32,
}

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
) {
    for (ent, mut buffer) in query.iter_mut() {
        for effect in buffer.vec.iter() {
            match effect {
                Effect::PoisonEffect(poison) => {
                    commands.entity(ent).insert(super::SlowPoisonDebuff {
                        effect_originator: *poison,
                        remaining_time: poison.duration,
                    });
                }
                Effect::StunEffect(stun) => {
                    commands
                        .entity(ent)
                        .insert(super::Stunned {
                            duration: stun.duration,
                        })
                        .remove::<super::actions::PerformingActionState>();
                }
                Effect::DamageEffect(damage_instance) => {
                    if let Ok(mut damages) = damage_query.get_mut(ent) {
                        damages.damages.push(*damage_instance);
                    }
                }
                _ => (),
            }
        }

        buffer.vec.clear();
    }
}

pub fn reset_stat_buffs() {
    //for each stat, set to max/default
}

pub fn apply_stat_buffs() {
    // Foreach statbuff, target_entity
    // try get stat and apply effect ()
}