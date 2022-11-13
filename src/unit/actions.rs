use bevy_ecs::prelude::*;
use gdnative::prelude::*;

#[derive(Component)]
pub struct ActionEntity{}

#[derive(Component)]
pub struct ApplyNow{}

#[derive(Component)]
pub struct Cooldown(f32);

#[derive(Component)]
pub struct PotentialTargets {
    targets: Vec<Entity>,
}

#[derive(Component)]
pub struct Caster{
    entity: Entity
}


#[derive(Component)]
pub struct TargetEntity {
    entity: Entity,
}


#[derive(Component)]
pub struct TargetPosition {
    pos: Position,
}

#[derive(Component)]
pub struct ChannelingDetails {
    total_time_channeled: f32
}

#[derive(Component)]
pub struct TargetsEnemies{}

#[derive(Component)]
pub struct TargetsAllies{}

#[derive(Component)]
pub struct Range(f32);

#[derive(Component)]
pub struct Splash{radius: f32, effect_ratio: f32}

#[derive(Component)]
pub struct Cleave{angle_degrees: f32, effect_ratio: f32}

#[derive(Component)]
pub struct OnHitEffects {
    apply_components: Vec<Component>
}

#[derive(Component)]
pub struct Projectile {
    pub target: Entity,
    pub target_pos: Vector2,
}


pub fn action_cooldown(mut commands: Commands, mut query: Query<(Entity, &mut Cooldown)>, delta: Res<DeltaPhysics>) {
    for (ent, mut cooldown) in query.iter_mut() {
        cooldown.0 -= delta.seconds;
        if cooldown.0 <= 0.0 {
            commands.entity(ent).remove::<Cooldown>();
        }
    }
}

pub fn channeling_details(mut query: Query<&mut ChannelingDetails>, delta: Res<DeltaPhysics>) {
    for mut channel in query.iter_mut() {
        channel.0 += delta.seconds;
    }
}



pub fn apply_effects(mut commands: Commands, query: Query<&OnHitEffects, (With<ActionEntity>, With<ApplyNow>)) {
    for effects in query.iter() {

    }
}




