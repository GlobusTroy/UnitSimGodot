use crate::physics::*;
use bevy_ecs::prelude::*;
use gdnative::{api::VisualServer, prelude::*};

#[derive(Component)]
pub struct Renderable {
    pub canvas_item_rid: Rid,
}

#[derive(Default)]
pub struct Delta {
    pub seconds: f32,
}

pub fn update_canvas_items(query: Query<(&Position, &Renderable)>) {
    for (position, renderable) in &query {
        unsafe {
            VisualServer::godot_singleton().canvas_item_set_transform(
                renderable.canvas_item_rid,
                Transform2D::IDENTITY.translated(position.pos),
            );
            //VisualServer::godot_singleton().canvas_item_set_modulate(renderable.canvas_item_rid, color)
        }
    }
}
