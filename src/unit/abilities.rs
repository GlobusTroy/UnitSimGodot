use bevy_ecs::prelude::*;
use gdnative::prelude::*;

use crate::{graphics::{NewCanvasItemDirective, animation::{AnimatedSprite, PlayAnimationDirective}, ScaleSprite, FlippableSprite}, physics::{Position, DeltaPhysics}, util::ExpirationTimer, boid::BoidParams};

use super::{Stunned, SlowPoisonEffect, Casting, Channeling, Hitpoints};


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
pub struct SlowPoisonAttack{
    pub duration: f32,
    pub percent_damage: f32,
    pub speed_multiplier: f32,
}


pub fn casting_state(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Casting, Option<&mut FlippableSprite>)>,
    pos_query: Query<&Position>,
    mut poison_query: Query<(&SlowPoisonEffect, &mut BoidParams)>,
    mut heal_query: Query<(&Position, &mut Hitpoints)>,
    delta: Res<DeltaPhysics>,
) {
    for (entity, mut casting, flippable_option) in query.iter_mut() {
        let ability_clone = casting.ability.clone();
        if let super::UnitAbility::Cleanse(cleanse) = ability_clone {
            // Impact hit -> apply cleanse 
            if casting.channeling_time < cleanse.impact_time
                && casting.channeling_time + delta.seconds >= cleanse.impact_time
            {
                // Guardrail against removed entities
                if let Ok(position) = pos_query.get(casting.target) {
                    commands.entity(casting.target).remove::<Stunned>();
                    commands.entity(casting.target).remove::<SlowPoisonEffect>();

                    let mut animated_sprite = AnimatedSprite::default();
                    animated_sprite.texture = cleanse.effect_texture;
                    commands
                        .spawn()
                        .insert(NewCanvasItemDirective {})
                        .insert(animated_sprite)
                        .insert(Position { pos: position.pos })
                        .insert(ExpirationTimer(1.5))
                        .insert(ScaleSprite(Vector2 {
                            x: 0.75,
                            y: 0.75,
                        }))
                        .insert(PlayAnimationDirective {
                            animation_name: "death".to_string(),
                            is_one_shot: true,
                        });
                }

                if let Ok((poison, mut boid)) = poison_query.get_mut(casting.target) {
                    boid.max_speed /= poison.effect_originator.speed_multiplier;
                }

               
            }
            // End casting state
            if casting.channeling_time < cleanse.swing_time
                && casting.channeling_time + delta.seconds >= cleanse.swing_time
            {
                commands.entity(entity).remove::<Casting>();
                commands.entity(entity).remove::<Channeling>();
            }
        }

        if let super::UnitAbility::Heal(heal) = ability_clone {
            // Impact hit -> apply heal 
            if casting.channeling_time < heal.impact_time
                && casting.channeling_time + delta.seconds >= heal.impact_time
            {
                // Guardrail against removed entities
                if let Ok((position, mut hitpoints)) = heal_query.get_mut(casting.target) {

                    hitpoints.hp = hitpoints.max_hp.min(hitpoints.hp + heal.heal_amount);
                    let mut animated_sprite = AnimatedSprite::default();
                    animated_sprite.texture = heal.effect_texture;
                    commands
                        .spawn()
                        .insert(NewCanvasItemDirective {})
                        .insert(animated_sprite)
                        .insert(Position { pos: position.pos })
                        .insert(ExpirationTimer(1.5))
                        .insert(ScaleSprite(Vector2 {
                            x: 0.75,
                            y: 0.75,
                        }))
                        .insert(PlayAnimationDirective {
                            animation_name: "death".to_string(),
                            is_one_shot: true,
                        });
                }

            }
            // End casting state
            if casting.channeling_time < heal.swing_time
                && casting.channeling_time + delta.seconds >= heal.swing_time
            {
                commands.entity(entity).remove::<Casting>();
                commands.entity(entity).remove::<Channeling>();
            }
        }

        casting.channeling_time += delta.seconds;

        if let Some(mut flipper) = flippable_option {
            if let Ok(attacker_pos) = pos_query.get(entity) {
                if let Ok(target_pos) = pos_query.get(casting.target) {
                    flipper.is_flipped = attacker_pos.pos.x > target_pos.pos.x;
                }
            }
        }
    }
}