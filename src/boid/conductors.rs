use bevy_ecs::prelude::*;

use super::*;

#[derive(Component)]
pub struct MeleeRushdownConductor {
    charge_at_enemy_boid: ChargeAtEnemyBoid,
    seek_enemies_boid: SeekEnemiesBoid,
    is_charging: bool,
}

#[derive(Component)]
pub struct KiteNearestConductor {
    pub kiting_boid: KiteNearestEnemyBoid,
    pub seek_enemies_boid: SeekEnemiesBoid,
    pub is_kiting: bool,
}

pub fn kite_conductor(
    mut commands: Commands,
    spatial: Res<SpatialNeighborsCache>,
    mut query: Query<(Entity, &mut KiteNearestConductor), Without<PerformingActionState>>,
) {
    for (entity, mut conductor) in query.iter_mut() {
        let neighbors_option = spatial.get_neighbors(&entity, conductor.kiting_boid.kite_radius);
        if let Some(neighbors) = neighbors_option {
            if neighbors.is_empty() {
                if conductor.is_kiting {
                    conductor.is_kiting = false;
                    commands
                        .entity(entity)
                        .remove::<KiteNearestEnemyBoid>()
                        .insert(conductor.seek_enemies_boid);
                }
            } else {
                if !conductor.is_kiting {
                    conductor.is_kiting = true;
                    commands
                        .entity(entity)
                        .remove::<SeekEnemiesBoid>()
                        .insert(conductor.kiting_boid);
                }
            }
        } else {
            if conductor.is_kiting {
                conductor.is_kiting = false;
                commands
                    .entity(entity)
                    .remove::<KiteNearestEnemyBoid>()
                    .insert(conductor.seek_enemies_boid);
            }
        }
    }
}
