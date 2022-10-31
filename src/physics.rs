use bevy_ecs::prelude::*;
use gdnative::prelude::*;
use spatial_structures::*;

use crate::util::*;

pub mod collisions;
pub mod spatial_structures;

#[derive(Default)]
pub struct DeltaPhysics {
    pub seconds: f32,
}

#[derive(Component)]
pub struct Position {
    pub pos: Vector2,
}

#[derive(Component)]
pub struct Velocity {
    pub v: Vector2,
}

#[derive(Component)]
pub struct Radius {
    pub r: f32,
}

#[derive(Component)]
pub struct Mass(pub f32);

#[derive(Component)]
pub struct Elasticity(pub f32); // 0 to 1

#[derive(Component)]
pub struct AppliedForces(pub Vector2);

pub fn physics_integrate(
    mut query: Query<(
        &mut Position,
        &mut Velocity,
        Option<&mut AppliedForces>,
        Option<&Mass>,
    )>,
    delta: Res<DeltaPhysics>,
) {
    for (mut position, mut velocity, force_option, mass_option) in &mut query {
        if let Some(mut force) = force_option {
            let force_vec = force.0;
            let mass = match mass_option {
                // Default mass = 1
                Some(mass_component) => mass_component.0,
                None => 1.,
            };

            velocity.v += force_vec / mass * delta.seconds;
            force.0 = Vector2::ZERO;
        }

        position.pos += velocity.v * delta.seconds;
    }
}
