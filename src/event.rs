pub enum EventCue {
    Audio(AudioCue),
    Damage(DamageCue),
}
pub struct DamageCue {
    pub damage: f32,
    pub damage_type: String,
    pub location: gdnative::prelude::Vector2,
}

pub struct AudioCue {
    pub event: String,
    pub location: gdnative::prelude::Vector2,
    pub texture: gdnative::prelude::Rid,
}

pub struct EventQueue(pub Vec<EventCue>);
