pub struct EventCue {
    pub event: String,
    pub location: gdnative::prelude::Vector2,
    pub texture: gdnative::prelude::Rid
}

pub struct EventQueue(pub Vec<EventCue>);
