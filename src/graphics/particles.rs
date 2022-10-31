use std::collections::HashMap;

use bevy_ecs::prelude::*;
use gdnative::{api::VisualServer, prelude::*};

use super::Renderable;

#[derive(Component, Clone)]
pub struct NewParticleEffectDirective {
    pub effect_name: String,
    pub position: Vector2
}

#[derive(Copy, Clone, Debug)]
pub struct ParticleEffect {
    pub effect_rid: Rid,
    pub texture_rid: Rid
}

#[derive(Debug, Clone)]
pub struct ParticleLibrary {
    pub map: HashMap<String, ParticleEffect>,
}

impl ParticleLibrary {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

pub fn execute_new_particle_effect_directive(
    query: Query<(&Renderable, &NewParticleEffectDirective)>,
    library: Option<Res<ParticleLibrary>>,
) {
    if let None = library {
        return;
    }
    let library = library.unwrap();
    for (renderable, effect) in query.iter() {
        if let Some(particles) = library.map.get(&effect.effect_name) {
            unsafe {
                let server = VisualServer::godot_singleton();
                server.canvas_item_add_particles(
                    renderable.canvas_item_rid,
                    particles.effect_rid,
                    particles.texture_rid,
                    Rid::default(),
                );
                server.particles_request_process(particles.effect_rid);
                server.particles_set_emitting(particles.effect_rid, true);
            }
        }
    }
}
