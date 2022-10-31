use bevy_ecs::prelude::*;
use boid::conductors::kite_conductor;
use gdnative::{api::VisualServer, prelude::*};

mod boid;
use boid::*;

mod graphics;
use graphics::*;

mod unit;
use graphics::particles::NewParticleEffectDirective;
use unit::*;

mod physics;
use physics::*;
use physics::collisions::*;
use physics::spatial_structures::*;

mod util;
use util::expire_entities;

#[derive(NativeClass)]
#[inherit(Node2D)]
pub struct ECSWorld {
    world: bevy_ecs::prelude::World,
    schedule: Schedule,
    schedule_logic: Schedule,
    canvas_item: Rid,
    clock: u64,

    terrain_map: TerrainMap,

    unit_blueprints: Vec<UnitBlueprint>,
    animation_library: animation::AnimationLibrary,
    particle_library: particles::ParticleLibrary,

    #[property]
    running: bool,

    #[property]
    draw_debug: bool,
}

pub struct Clock(pub u64);

#[methods]
impl ECSWorld {
    fn new(base: &Node2D) -> Self {
        let mut schedule_physics = Schedule::default();
        schedule_physics.add_stage(
            "integrate_physics",
            SystemStage::parallel()
                .with_system(physics::physics_integrate)
                .with_system(remove_stuns)
                .with_system(melee_weapon_cooldown)
                .with_system(projectile_weapon_cooldown)
                .with_system(attacking_state)
                .with_system(apply_damages)
                .with_system(expire_entities),
        );
        schedule_physics.add_stage(
            "build_spatial_hash",
            SystemStage::parallel().with_system(build_spatial_hash_table),
        );
        schedule_physics.add_stage(
            "detect_collisions",
            SystemStage::parallel()
                .with_system(detect_collisions)
                .with_system(build_spatial_neighbors_cache)
                .with_system(build_flow_fields),
        );

        let mut schedule_resolution = Schedule::default();
        schedule_resolution.add_stage(
            "resolve_collisions",
            SystemStage::parallel().with_system(resolve_collisions_iteration),
        );
        schedule_resolution.add_stage(
            "detect_collisions",
            SystemStage::parallel().with_system(detect_collisions),
        );
        let collisions_stage = CollisionStage {
            schedule: schedule_resolution,
            max_iterations: 8,
        };

        schedule_physics.add_stage("resolve_collisions", collisions_stage);

        let mut schedule_behavior = Schedule::default();
        schedule_behavior.add_stage(
            "conductors",
            SystemStage::parallel().with_system(kite_conductor),
        );
        schedule_behavior.add_stage(
            "behavior+boid_steer",
            SystemStage::parallel()
                .with_system(separation_boid)
                .with_system(stopping_boid)
                .with_system(seek_enemies_boid)
                .with_system(avoid_walls_boid)
                .with_system(cohesion_boid)
                .with_system(vector_alignment_boid)
                .with_system(charge_at_enemy_boid)
                .with_system(kite_enemies_boid)
                .with_system(attack_enemy_behavior)
                .with_system(update_targeted_projectiles),
        );
        schedule_behavior.add_stage(
            "execute_directives+boid_normalize",
            SystemStage::parallel()
                .with_system(boid_apply_params)
                .with_system(execute_attack_target_directive)
                .with_system(animation::execute_play_animation_directive),
        );

        let mut schedule_logic = Schedule::default();
        schedule_logic.add_stage("physics", schedule_physics);
        schedule_logic.add_stage("behavior", schedule_behavior);

        let mut schedule = Schedule::default();

        schedule.add_stage(
            "graphics",
            SystemStage::parallel().with_system(update_canvas_items),
        );
        schedule.add_stage(
            "animate",
            SystemStage::parallel().with_system(animation::animate_sprites),
        );

        let world = World::new();

        Self {
            world: world,
            schedule_logic: schedule_logic,
            schedule: schedule,
            canvas_item: base.get_canvas_item(),
            clock: 0,
            terrain_map: TerrainMap::default(),
            unit_blueprints: Vec::new(),
            animation_library: animation::AnimationLibrary::new(),
            particle_library: particles::ParticleLibrary::new(),
            running: false,
            draw_debug: false,
        }
    }

    #[method]
    fn _ready(&mut self, #[base] _base: &Node2D) {
        let radii = [4., 16., 64., 256., 512.];
        self.world
            .insert_resource(spatial_structures::SpatialNeighborsRadii(Box::new(radii)));
    }

    #[method]
    fn add_unit_animation_set(
        &mut self,
        texture: Rid,
        animation_name: String,
        my_rect: Rect2,
        src_rects: Vec<Rect2>,
        anim_speed: f32,
    ) {
        let animation_set = animation::AnimationSet {
            sprite_rect: my_rect,
            rect_vec: src_rects,
            speed: anim_speed,
        };
        unsafe {
            self.animation_library
                .set_animation(texture, animation_name, animation_set)
        }
        self.world.insert_resource(self.animation_library.clone());
    }

    #[method]
    fn add_particle_effect(&mut self, effect_name: String, effect_rid: Rid, texture_rid: Rid) {
        let particle_effect = particles::ParticleEffect{effect_rid: effect_rid, texture_rid: texture_rid};
        self.particle_library.map.insert(effect_name, particle_effect);
        self.world.insert_resource(self.particle_library.clone());
    }

    #[method]
    fn add_unit_blueprint(
        &mut self,
        texture: Rid,
        hitpoints: f32,
        radius: f32,
        mass: f32,
        movespeed: f32,
        acceleration: f32,
    ) -> usize {
        let blueprint =
            UnitBlueprint::new(texture, hitpoints, radius, mass, movespeed, acceleration);
        self.unit_blueprints.push(blueprint);
        return self.unit_blueprints.len() - 1;
    }

    #[method]
    fn add_melee_weapon_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
    ) {
        let weapon = Weapon::Melee(MeleeWeapon {
            damage: damage,
            range: range,
            cooldown_time: cooldown,
            impact_time: impact_time,
            full_swing_time: swing_time,
            time_until_weapon_cooled: 0.0,
        });
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_weapon(weapon);
        }
    }

    #[method]
    fn add_projectile_weapon_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        projectile_speed: f32,
        projectile_scale: f32,
        projectile_texture_rid: Rid,
    ) {
        let weapon = Weapon::Projectile(ProjectileWeapon {
            damage: damage,
            range: range,
            cooldown_time: cooldown,
            impact_time: impact_time,
            full_swing_time: swing_time,
            time_until_weapon_cooled: 0.0,
            projectile_speed: projectile_speed,
            projectile_scale: projectile_scale,
            projectile_texture: projectile_texture_rid,
        });
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_weapon(weapon);
        }
    }

    #[method]
    fn set_tile_size(&mut self, tile_size: f32) {
        self.terrain_map.cell_size = tile_size;
    }

    #[method]
    fn set_bounds_by_tiles(&mut self, x_bound: i32, y_bound: i32) {
        self.terrain_map.max_bounds = SpatialHashCell(x_bound, y_bound);
    }

    #[method]
    fn set_bounds_by_space(&mut self, x_bound: f32, y_bound: f32) {
        self.terrain_map.max_bounds = SpatialHashCell(
            (x_bound / self.terrain_map.cell_size) as i32,
            (y_bound / self.terrain_map.cell_size) as i32,
        );
    }

    #[method]
    fn set_tile(&mut self, x: i32, y: i32, pathing_mask: usize, movement_cost: f32) {
        self.terrain_map.map.insert(
            SpatialHashCell(x, y),
            TerrainCell {
                pathable_mask: pathing_mask,
                movement_cost: movement_cost,
            },
        );
    }

    #[method]
    fn set_default_tile(&mut self, pathing_mask: usize, movement_cost: f32) {
        self.terrain_map.default_cell = TerrainCell {
            pathable_mask: pathing_mask,
            movement_cost: movement_cost,
        };
    }

    #[method]
    fn set_out_of_bounds_tile(&mut self, pathing_mask: usize, movement_cost: f32) {
        self.terrain_map.out_of_bounds_cell = TerrainCell {
            pathable_mask: pathing_mask,
            movement_cost: movement_cost,
        };
    }

    #[method]
    fn spawn_unit(&mut self, team_id: usize, position: Vector2, blueprint_id: usize) -> u32 {
        let blueprint = self.unit_blueprints.get(blueprint_id).unwrap();
        let visual_server = unsafe { VisualServer::godot_singleton() };
        let canvas_item_rid = visual_server.canvas_item_create();
        unsafe {
            visual_server.canvas_item_set_parent(canvas_item_rid, self.canvas_item);
        };

        let ent = self
            .world
            .spawn()
            .insert(TeamAlignment {
                alignment: TeamValue::Team(team_id),
            })
            .insert(Position { pos: position })
            .insert(Velocity { v: Vector2::ZERO })
            .insert(Radius {
                r: blueprint.radius,
            })
            .insert(Mass(blueprint.mass))
            .insert(BoidParams {
                max_force: blueprint.acceleration * blueprint.mass,
                max_speed: blueprint.movespeed,
            })
            .insert(SeparationBoid {
                avoidance_radius: 4.,
                multiplier: 2.,
            })
            .insert(CohesionBoid {
                cohesion_radius: 8.,
                multiplier: 1.,
            })
            .insert(VectorAlignmentBoid {
                alignment_radius: 8.,
                multiplier: 2.,
            })
            .insert(AvoidWallsBoid {
                avoidance_radius: 4.,
                multiplier: 2.,
                cell_size_multiplier: 0.55,
            })
            .insert(StoppingBoid { multiplier: 10. })
            .insert(SeekEnemiesBoid { multiplier: 1. })
            .insert(AppliedBoidForces(Vector2::ZERO))
            .insert(AppliedForces(Vector2::ZERO))
            .insert(animation::AnimatedSprite {
                texture: blueprint.texture,
                animation_name: "run".to_string(),
                animation_index: 0,
                animation_speed: unsafe {
                    self.animation_library
                        .get_animation_speed(blueprint.texture, "run".to_string())
                },
                animation_length: unsafe {
                    self.animation_library
                        .get_animation_length(blueprint.texture, "run".to_string())
                },
                animation_time_since_change: 0.0,
                is_one_shot: false,
            })
            .insert(FlippableSprite {
                is_flipped: false,
                flip_speed: 2.,
            })
            .insert(Renderable {
                canvas_item_rid: canvas_item_rid,
            })
            .insert(AppliedDamage {
                damages: Vec::new(),
            })
            .insert(AttackEnemyBehavior {})
            .insert(Hitpoints {
                max_hp: blueprint.hitpoints,
                hp: blueprint.hitpoints,
            })
            .id();

        // Insert Weapons
        for weapon in blueprint.weapons.iter() {
            if let Weapon::Melee(melee_weapon) = weapon {
                self.world
                    .entity_mut(ent)
                    .insert(*melee_weapon)
                    .insert(ChargeAtEnemyBoid {
                        charge_radius: melee_weapon.range * 3.,
                        multiplier: 5.,
                    })
                    .insert(SpatialAwareness {
                        radius: blueprint.radius + self.terrain_map.cell_size,
                    });
            } else if let Weapon::Projectile(projectile_weapon) = weapon {
                self.world
                    .entity_mut(ent)
                    .insert(*projectile_weapon)
                    .insert(KiteNearestEnemyBoid {
                        multiplier: 5.,
                        kite_radius: projectile_weapon.range,
                    })
                    .insert(SpatialAwareness {
                        radius: (blueprint.radius + self.terrain_map.cell_size)
                            .max(projectile_weapon.range * 1.5 + blueprint.radius),
                    });
            }
        }

        return ent.id();
    }

    #[method]
    #[profiled]
    fn _physics_process(&mut self, delta: f32) {
        if !self.running {
            return;
        }

        self.world.insert_resource(DeltaPhysics { seconds: delta });
        self.world.insert_resource(Clock(self.clock));
        self.world.insert_resource(self.terrain_map.clone());
        self.clock += 1;
        self.schedule_logic.run(&mut self.world);
    }

    fn _process_new_canvas_items(&mut self) {
        let mut entities: Vec<Entity> = Vec::new();
        for (entity, _directive) in self
            .world
            .query::<(Entity, &NewCanvasItemDirective)>()
            .iter(&self.world)
        {
            entities.push(entity);
        }
        for entity in entities {
            unsafe {
                let server = VisualServer::godot_singleton();
                let canvas_item_rid = server.canvas_item_create();
                server.canvas_item_set_parent(canvas_item_rid, self.canvas_item);
                self.world.entity_mut(entity).insert(Renderable {
                    canvas_item_rid: canvas_item_rid,
                });
            }
            self.world
                .entity_mut(entity)
                .remove::<NewCanvasItemDirective>();
        }
    }

    // fn _process_new_particle_effects(&mut self) {
    //     let mut entities: Vec<Entity> = Vec::new();
    //     for (entity, directive) in self
    //         .world
    //         .query::<(Entity, &NewParticleEffectDirective)>()
    //         .iter(&self.world)
    //     {
    //         entities.push(entity);
    //         if let Some(effect) = self.particle_library.map.get(&directive.effect_name) {
    //             unsafe {
    //                 let server = VisualServer::godot_singleton();
    //                 let canvas_item_rid = self.canvas_item;
    //                 //server.particles_set_emission_transform(effect.effect_rid, Transform::IDENTITY.translated(Vector3{x:directive.position.x, y:directive.position.y, z:0.}));
    //                 server.canvas_item_add_particles(canvas_item_rid, effect.effect_rid, effect.texture_rid, Rid::default()); 
    //             }
    //         }
    //     }
    //     for entity in entities {
    //         self.world
    //             .entity_mut(entity)
    //             .remove::<NewParticleEffectDirective>();
    //     }
    // }

    #[method]
    #[profiled]
    fn _process(&mut self, #[base] base: TRef<Node2D>, delta: f32) {
        if !self.running {
            return;
        }
        self.world.insert_resource(Delta { seconds: delta });
        

        self._process_new_canvas_items();
        //self._process_new_particle_effects();
        let mut entities: Vec<Entity> = Vec::new();
        for (entity, item) in self
            .world
            .query::<(Entity, &CleanupCanvasItem)>()
            .iter(&self.world)
        {
            unsafe {
                let server = VisualServer::godot_singleton();
                let canvas_item_rid = item.0;
                server.canvas_item_clear(canvas_item_rid);
            }
            entities.push(entity);
        }

        for entity in entities {
            self.world.entity_mut(entity).despawn();
        }

        self.schedule.run(&mut self.world);
        if (self.clock % 24) == 0 {
            base.update();
        }
    }

    #[method]
    fn _draw(&mut self, #[base] base: TRef<Node2D>) {
        if self.draw_debug {
            debug_draw::draw_terrain_map(self, base);
            debug_draw::draw_integration_values(self, base);
            debug_draw::draw_flow_field(self, base);
        }
    }
}

fn init(handle: InitHandle) {
    handle.add_class::<ECSWorld>();
}

godot_init!(init);
