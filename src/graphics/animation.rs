use std::collections::HashMap;

use bevy_ecs::prelude::*;
use gdnative::{api::VisualServer, prelude::*};

use crate::{
    physics::{Radius, Velocity},
    unit::{Hitpoints, TeamAlignment, TeamValue, BlueprintId},
};

use super::{Delta, Renderable, ScaleSprite};

#[derive(Debug, Clone)]
pub struct AnimationSet {
    pub rect_vec: Vec<Rect2>,
    pub sprite_rect: Rect2,
    pub speed: f32,
}

#[derive(Debug, Clone)]
pub struct AnimationLibrary {
    map: HashMap<i32, HashMap<String, AnimationSet>>,
}

impl AnimationLibrary {
    pub fn new() -> AnimationLibrary {
        Self {
            map: HashMap::new(),
        }
    }

    pub unsafe fn set_animation(
        &mut self,
        texture: Rid,
        animation_name: String,
        animation_set: AnimationSet,
    ) {
        if self.map.contains_key(&texture.get_id()) {
            let unit_anims = self.map.get_mut(&texture.get_id()).unwrap();
            unit_anims.insert(animation_name, animation_set);
        } else {
            let mut unit_anims = HashMap::new();
            unit_anims.insert(animation_name, animation_set);
            self.map.insert(texture.get_id(), unit_anims);
        }
    }

    pub unsafe fn get_animation_speed(&self, texture: Rid, animation_name: String) -> f32 {
        let unit_anims = self.map.get(&texture.get_id());
        if let Some(anims) = unit_anims {
            if let Some(animation) = anims.get(&animation_name) {
                return animation.speed;
            }
        }
        return 0.0;
    }

    pub unsafe fn get_animation_length(&self, texture: Rid, animation_name: String) -> usize {
        let unit_anims = self.map.get(&texture.get_id());
        if let Some(anims) = unit_anims {
            if let Some(animation) = anims.get(&animation_name) {
                return animation.rect_vec.len();
            }
        }
        return 0;
    }

    pub unsafe fn get_animation_sprite_rect(&self, texture: Rid, animation_name: String) -> Rect2 {
        let unit_anims = self.map.get(&texture.get_id());
        if let Some(anims) = unit_anims {
            if let Some(animation) = anims.get(&animation_name) {
                return animation.sprite_rect;
            }
        }
        return Rect2 {
            position: Vector2 { x: -16., y: -32. },
            size: Vector2 { x: 32., y: 32. },
        };
    }

    pub unsafe fn get_animation_rect(
        &self,
        texture: Rid,
        animation_name: String,
        index: usize,
    ) -> Rect2 {
        let unit_anims = self.map.get(&texture.get_id());
        if let Some(anims) = unit_anims {
            if let Some(animation) = anims.get(&animation_name) {
                if let Some(rect) = animation.rect_vec.get(index) {
                    return *rect;
                }
            }
        }
        return Rect2 {
            position: Vector2 { x: 0., y: 0. },
            size: Vector2 { x: 32., y: 32. },
        };
    }
}

#[derive(Component, Default)]
pub struct AnimatedSprite {
    pub texture: Rid,
    pub animation_name: String,
    pub animation_index: usize,
    pub animation_speed: f32,
    pub animation_time_since_change: f32,
    pub animation_length: usize,
    pub is_one_shot: bool,
}

impl AnimatedSprite {
    pub fn new(texture: Rid) -> Self {
        let mut new = AnimatedSprite::default();
        new.texture = texture;
        new
    }
}

#[derive(Component)]
pub struct PlayAnimationDirective {
    pub animation_name: String,
    pub is_one_shot: bool,
}

pub fn execute_play_animation_directive(
    mut commands: Commands,
    mut query: Query<(Entity, &PlayAnimationDirective, &mut AnimatedSprite)>,
    event_query: Query<&crate::physics::Position>,
    library: Res<AnimationLibrary>,
    mut events: ResMut<crate::event::EventQueue>,
) {
    for (entity, directive, mut sprite) in query.iter_mut() {
        sprite.animation_name = directive.animation_name.to_string();
        sprite.is_one_shot = directive.is_one_shot;
        sprite.animation_time_since_change = 0.0;
        sprite.animation_index = 0;
        unsafe {
            sprite.animation_speed =
                library.get_animation_speed(sprite.texture, sprite.animation_name.to_string());
            sprite.animation_length =
                library.get_animation_length(sprite.texture, sprite.animation_name.to_string());
        }

        if let Ok(pos) = event_query.get(entity) {
            events.0.push(crate::event::EventCue {
                event: directive.animation_name.clone(),
                location: pos.pos,
                texture: sprite.texture
            });
        }
       commands.entity(entity).remove::<PlayAnimationDirective>();
    }
}

pub fn animate_sprites(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut AnimatedSprite,
        &Renderable,
        Option<&ScaleSprite>,
        Option<&Radius>,
        Option<&TeamAlignment>,
        Option<&Hitpoints>,
        Option<&super::AlphaSprite>,
        Option<&super::ModulateSprite>,
    )>,
    delta: Res<Delta>,
    library: Res<AnimationLibrary>,
) {
    for (
        entity,
        mut sprite,
        renderable,
        scale_option,
        radius_option,
        alignment_option,
        hitpoints_option,
        alpha_option,
        modulate_option,
    ) in query.iter_mut()
    {
        //Animate
        sprite.animation_time_since_change += delta.seconds;
        let time_per_frame = 1. / sprite.animation_speed;
        while sprite.animation_time_since_change >= time_per_frame {
            sprite.animation_time_since_change -= time_per_frame;
            sprite.animation_index += 1;
        }

        if sprite.is_one_shot && sprite.animation_index >= sprite.animation_length {
            if sprite.animation_name == "death".to_string() {
                unsafe {
                    VisualServer::godot_singleton().canvas_item_set_modulate(
                        renderable.canvas_item_rid,
                        Color {
                            r: 1.,
                            g: 1.,
                            b: 1.,
                            a: 0.25,
                        },
                    )
                }
                continue;
            }
            commands.entity(entity).insert(PlayAnimationDirective {
                animation_name: "idle".to_string(),
                is_one_shot: false,
            });
        } else {
            sprite.animation_index %= sprite.animation_length;
        }

        let rect = unsafe {
            library.get_animation_rect(
                sprite.texture,
                sprite.animation_name.to_string(),
                sprite.animation_index,
            )
        };
        let mut self_rect = unsafe {
            library.get_animation_sprite_rect(sprite.texture, sprite.animation_name.to_string())
        };

        if let Some(scale) = scale_option {
            let transform = Transform2D::IDENTITY.scaled(scale.0);
            self_rect.position = transform.xform_inv(self_rect.position);
            self_rect.size = transform.xform_inv(self_rect.size);
        }

        unsafe {
            let server = VisualServer::godot_singleton();

            server.canvas_item_clear(renderable.canvas_item_rid);
            if let Some(radius) = radius_option {
                if let Some(team) = alignment_option {
                    if let Some(hitpoints) = hitpoints_option {
                        let mut green = 0.0;
                        let mut blue = 0.0;
                        let mut red = 0.0;
                        match team.alignment {
                            TeamValue::Team(1) => {
                                red = 1.0;
                            }
                            TeamValue::Team(2) => {
                                blue = 1.0;
                            }
                            TeamValue::NeutralHostile => {
                                green = 1.0;
                                blue = 0.75;
                                red = 0.75;
                            }
                            _ => {}
                        };
                        server.canvas_item_add_circle(
                            renderable.canvas_item_rid,
                            Vector2::ZERO,
                            (1.25 * radius.r) as f64,
                            Color {
                                r: red,
                                g: green as i32 as f32,
                                b: blue as i32 as f32,
                                a: 0.1,
                            },
                        );
                        server.canvas_item_add_circle(
                            renderable.canvas_item_rid,
                            Vector2::ZERO,
                            (radius.r * (0.15 + 0.85 * (hitpoints.hp / hitpoints.max_hp))) as f64,
                            Color {
                                r: red,
                                g: green as i32 as f32,
                                b: blue as i32 as f32,
                                a: 0.3 + 0.2 * (hitpoints.hp / hitpoints.max_hp),
                            },
                        );
                    }
                }
            }

            let mut color = Color {
                r: 1.,
                g: 1.,
                b: 1.,
                a: 1.,
            };

            if let Some(alpha) = alpha_option {
                color.a = alpha.0;
            }
            if let Some(modulate) = modulate_option {
                color.r = modulate.r;
                color.g = modulate.g;
                color.b = modulate.b;
            }

            server.canvas_item_add_texture_rect_region(
                renderable.canvas_item_rid,
                self_rect,
                sprite.texture,
                rect,
                color,
                false,
                Rid::default(),
                false,
            );
        }
    }
}
