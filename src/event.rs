pub enum EventCue {
    Audio(AudioCue),
    Damage(DamageCue),
}

pub struct EventEntityData {
    pub ent: bevy_ecs::prelude::Entity,
    pub blueprint: crate::unit::BlueprintId,
    pub team: crate::unit::TeamAlignment,
}
pub struct DamageCue {
    pub damage: f32,
    pub damage_type: String,
    pub location: gdnative::prelude::Vector2,
    pub attacker: EventEntityData,
    pub receiver: EventEntityData,
}

pub struct AudioCue {
    pub event: String,
    pub location: gdnative::prelude::Vector2,
    pub texture: gdnative::prelude::Rid,
}

pub struct EventQueue(pub Vec<EventCue>);
