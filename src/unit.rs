use bevy_ecs::prelude::*;
use gdnative::prelude::*;

#[derive(Debug)]
pub enum TeamValue {
    NeutralPassive,
    NeutralHostile,
    Team(usize)
}

#[derive(Component)]
pub struct TeamAlignment {
    pub alignment: TeamValue 
}
