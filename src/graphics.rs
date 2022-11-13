use crate::{physics::*, unit::Channeling};
use bevy_ecs::prelude::*;
use gdnative::{api::VisualServer, prelude::*};

pub mod animation;
pub mod debug_draw;
pub mod particles;

#[derive(Component)]
pub struct Renderable {
    pub canvas_item_rid: Rid,
}

#[derive(Component)]
pub struct NewCanvasItemDirective {}

#[derive(Component)]
pub struct CleanupCanvasItem(pub Rid);

#[derive(Component)]
pub struct FlippableSprite {
    pub is_flipped: bool,
    pub flip_speed: f32,
}

#[derive(Component)]
pub struct ScaleSprite(pub Vector2);

#[derive(Default)]
pub struct Delta {
    pub seconds: f32,
}

pub fn update_canvas_items(
    mut query: Query<(
        &mut Renderable,
        &Position,
        Option<&Velocity>,
        Option<&mut FlippableSprite>,
        Option<&ScaleSprite>,
        Option<&Channeling>,
    )>,
) {
    for (renderable, position, velocity_option, flippable_option, scale_option, stunned_option) in
        query.iter_mut()
    {
        let mut transform = Transform2D::IDENTITY;

        if let Some(velocity) = velocity_option {
            if let Some(mut flippable) = flippable_option {
                if let None = stunned_option {
                    if velocity.v.x.abs() > flippable.flip_speed {
                        flippable.is_flipped = velocity.v.x < 0.0;
                    }
                }

                if flippable.is_flipped {
                    transform = transform.scaled(Vector2 { x: -1., y: 1. });
                }
            }
        }

        let mut scale_vec: Vector2 = Vector2::ONE;

        if let Some(scale) = scale_option {
            scale_vec = scale.0;
        }
        transform = transform.scaled(scale_vec);
        transform =
            transform.translated(transform.xform(position.pos) / (scale_vec.x * scale_vec.y));

        unsafe {
            let server = VisualServer::godot_singleton();
            server.canvas_item_set_transform(renderable.canvas_item_rid, transform);
        }
    }
}
