use bevy_ecs::prelude::*;
use boid::conductors::kite_conductor;
use gdnative::{api::VisualServer, prelude::*};

mod event;
use event::*;

mod boid;
use boid::*;

mod graphics;
use graphics::*;

mod unit;
use unit::*;

mod physics;
use physics::collisions::*;
use physics::spatial_structures::*;
use physics::*;

mod util;
use unit::abilities::CleanseAbility;
use unit::abilities::HealAbility;
use unit::abilities::MagicMissileAbility;
use unit::abilities::SlowPoisonAttack;
use unit::abilities::UnitAbility;

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

    #[property]
    victor: i32,
}

pub struct Clock(pub u64);

#[methods]
impl ECSWorld {
    fn new(base: &Node2D) -> Self {
        let mut schedule_physics = Schedule::default();
        schedule_physics.add_stage(
            "dispose1",
            SystemStage::parallel()
                .with_system(util::expire_entities)
                .with_system(unit::resolve_death)
                .with_system(effects::percent_cooldown_speedup),
        );
        schedule_physics.add_stage(
            "dispose2",
            SystemStage::parallel()
                .with_system(effects::buff_timer)
                .with_system(actions::action_cooldown),
        );
        schedule_physics.add_stage(
            "integrate",
            SystemStage::parallel().with_system(physics::physics_integrate),
        );
        schedule_physics.add_stage(
            "effects",
            SystemStage::parallel().with_system(effects::resolve_effects),
        );
        schedule_physics.add_stage(
            "effects2",
            SystemStage::parallel().with_system(effects::apply_teleport),
        );
        schedule_physics.add_stage(
            "apply",
            SystemStage::parallel()
                .with_system(effects::apply_stat_buffs)
                .with_system(effects::percent_damage_over_time)
                .with_system(effects::heal_over_time)
                .with_system(effects::apply_stun_buff)
                // BUILD SPATIAL HASH
                .with_system(build_spatial_hash_table),
        );
        schedule_physics.add_stage(
            "override",
            SystemStage::parallel()
                .with_system(effects::set_stats_directly)
                .with_system(util::copy_target_position)
                .with_system(apply_damages),
        );
        schedule_physics.add_stage(
            "detect_collisions",
            SystemStage::parallel()
                .with_system(detect_collisions)
                .with_system(build_spatial_neighbors_cache)
                .with_system(build_flow_fields),
        );

        // Iteratively resolve and check collisions with collision stage
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
            "behavior1",
            SystemStage::parallel().with_system(projectiles::projectile_homing),
        );
        schedule_behavior.add_stage(
            "target",
            SystemStage::parallel()
                .with_system(actions::target_units)
                .with_system(projectiles::projectile_contact)
                .with_system(boid::update_boid_params_to_stats)
                .with_system(kite_conductor),
        );
        schedule_behavior.add_stage(
            "boid/perform",
            SystemStage::parallel()
                .with_system(actions::performing_action_state)
                .with_system(separation_boid)
                .with_system(stopping_boid)
                .with_system(seek_enemies_boid)
                .with_system(avoid_walls_boid)
                .with_system(cohesion_boid)
                .with_system(vector_alignment_boid)
                .with_system(charge_at_enemy_boid)
                .with_system(kite_enemies_boid),
        );
        schedule_behavior.add_stage(
            "execute_directives+boid_normalize",
            SystemStage::parallel()
                .with_system(boid_apply_params)
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
            draw_debug: true,
            victor: -1,
        }
    }

    #[method]
    fn _ready(&mut self, #[base] base: &Node2D) {
        self.setup_event_cue_signal(base);
        self.world.insert_resource(event::EventQueue(Vec::new()));
        let radii = [4., 16., 64., 256., 512.];
        self.world
            .insert_resource(spatial_structures::SpatialNeighborsRadii(Box::new(radii)));
    }

    fn setup_event_cue_signal(&mut self, base: &Node2D) {
        base.add_user_signal("event_cue", VariantArray::default());
        base.add_user_signal("damage_cue", VariantArray::default());
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
        let particle_effect = particles::ParticleEffect {
            effect_rid: effect_rid,
            texture_rid: texture_rid,
        };
        self.particle_library
            .map
            .insert(effect_name, particle_effect);
        self.world.insert_resource(self.particle_library.clone());
    }

    #[method]
    fn add_unit_blueprint(
        &mut self,
        texture: Rid,
        hitpoints: f32,
        radius: f32,
        awareness: f32,
        mass: f32,
        movespeed: f32,
        acceleration: f32,
        armor: f32,
        magic_resist: f32,
    ) -> usize {
        let blueprint = UnitBlueprint::new(
            texture,
            hitpoints,
            radius,
            awareness,
            mass,
            movespeed,
            acceleration,
            armor,
            magic_resist,
        );
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
        cleave_degrees: f32,
    ) {
        let weapon = Weapon::Melee(MeleeWeapon {
            damage: damage,
            range: range,
            cooldown_time: cooldown,
            impact_time: impact_time,
            full_swing_time: swing_time,
            time_until_weapon_cooled: 0.0,
            cleave_degrees: cleave_degrees,
        });
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_weapon(weapon);
        }
    }

    #[method]
    fn add_slow_poison_to_blueprint(
        &mut self,
        blueprint_id: usize,
        percent_damage: f32,
        duration: f32,
        movement_multiplier: f32,
        texture: Rid,
    ) {
        let poison = SlowPoisonAttack {
            percent_damage: percent_damage,
            duration: duration,
            movement_debuff: movement_multiplier,
            poison_texture: texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::SlowPoison(poison));
        }
    }

    #[method]
    fn add_armor_debuff_to_blueprint(
        &mut self,
        blueprint_id: usize,
        armor_debuff: f32,
        magic_armor_debuff: f32,
        duration: f32,
        texture: Rid,
    ) {
        let poison = abilities::ArmorReductionAttack {
            armor_reduction: armor_debuff,
            magic_armor_reduction: magic_armor_debuff,
            duration: duration,
            texture: texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::ArmorReduction(poison));
        }
    }

    #[method]
    fn add_confusion_to_blueprint(
        &mut self,
        blueprint_id: usize,
        set_acceleration: f32,
        duration: f32,
        texture: Rid,
    ) {
        let confusion = abilities::ConfusionAttack {
            set_acceleration: set_acceleration,
            duration: duration,
            texture: texture,
        };

        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Confusion(confusion));
        }
    }

    #[method]
    fn add_stun_to_blueprint(&mut self, blueprint_id: usize, duration: f32, texture: Rid) {
        let stun = StunOnHitEffect {
            duration: duration,
            stun_texture: texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Stun(stun));
        }
    }

    #[method]
    fn add_antiheal_to_blueprint(
        &mut self,
        blueprint_id: usize,
        percent_heal_reduction: f32,
        duration: f32,
        texture: Rid,
    ) {
        let antiheal = AntihealOnHitEffect {
            duration: duration,
            texture: texture,
            percent_heal_reduction: percent_heal_reduction,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::AntiHeal(antiheal));
        }
    }

    #[method]
    fn add_cleanse_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let cleanse_ability = CleanseAbility {
            range: range,
            cooldown: cooldown,
            impact_time: impact_time,
            swing_time: swing_time,
            time_until_cleanse_cooled: 0.0,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Cleanse(cleanse_ability));
        }
    }

    #[method]
    fn add_heal_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        heal_amount: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let heal_ability = HealAbility {
            heal_amount: heal_amount,
            range: range,
            cooldown: cooldown,
            impact_time: impact_time,
            swing_time: swing_time,
            time_until_cooled: 0.0,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Heal(heal_ability));
        }
    }

    #[method]
    fn add_magic_missile_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let ability = MagicMissileAbility {
            damage: damage,
            range: range,
            cooldown: cooldown,
            impact_time: impact_time,
            swing_time: swing_time,
            time_until_cooled: 0.0,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::MagicMissile(ability));
        }
    }

    #[method]
    fn add_whirlwind_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
    ) {
        let ability = abilities::WhirlwindAbility {
            damage: damage,
            range: range,
            cooldown: cooldown,
            impact_time: impact_time,
            swing_time: swing_time,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Whirlwind(ability));
        }
    }

    #[method]
    fn add_overdrive_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        percent_cooldown_reduction: f32,
        range: f32,
        cooldown: f32,
        duration: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let ability = abilities::OverdriveAbility {
            percent_cooldown_speedup: percent_cooldown_reduction,
            range: range,
            cooldown: cooldown,
            duration: duration,
            impact_time: impact_time,
            swing_time: swing_time,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Overdrive(ability));
        }
    }

    #[method]
    fn add_buff_resistance_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        magic_armor_amount: f32,
        range: f32,
        duration: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let ability = abilities::BuffResistanceAbility {
            magic_armor_amount: magic_armor_amount,
            range: range,
            cooldown: cooldown,
            duration: duration,
            impact_time: impact_time,
            swing_time: swing_time,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::BuffResistance(ability));
        }
    }

    #[method]
    fn add_fortify_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        heal_immediate: f32,
        heal_over_time: f32,
        armor_amount: f32,
        range: f32,
        duration: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        effect_texture: Rid,
    ) {
        let ability = abilities::FortifyAbility {
            heal_immediate: heal_immediate,
            heal_over_time: heal_over_time,
            armor_amount: armor_amount,
            range: range,
            cooldown: cooldown,
            duration: duration,
            impact_time: impact_time,
            swing_time: swing_time,
            effect_texture: effect_texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Fortify(ability));
        }
    }

    #[method]
    fn add_backstab_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage: f32,
        range: f32,
        cooldown: f32,
        impact_time: f32,
        swing_time: f32,
        texture: Rid,
    ) {
        let ability = abilities::BackstabAbility {
            damage: damage,
            range: range,
            cooldown: cooldown,
            impact_time: impact_time,
            swing_time: swing_time,
            texture: texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::Backstab(ability));
        }
    }

    #[method]
    fn add_damagebuff_ability_to_blueprint(
        &mut self,
        blueprint_id: usize,
        damage_buff: f32,
        range: f32,
        cooldown: f32,
        duration: f32,
        impact_time: f32,
        swing_time: f32,
        texture: Rid,
    ) {
        let ability = abilities::DamageBuffAbility {
            damage: damage_buff,
            range: range,
            cooldown: cooldown,
            duration: duration,
            impact_time: impact_time,
            swing_time: swing_time,
            texture: texture,
        };
        if let Some(blueprint) = self.unit_blueprints.get_mut(blueprint_id) {
            blueprint.add_ability(UnitAbility::DamageBuff(ability));
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
        splash_radius: f32,
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
            splash_radius: splash_radius,
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

        let ent = self
            .world
            .spawn()
            .insert(BlueprintId(blueprint_id))
            .insert(NewCanvasItemDirective {})
            .insert(TeamAlignment {
                alignment: TeamValue::Team(team_id),
            })
            .insert(Position { pos: position })
            .insert(Velocity { v: Vector2::ZERO })
            .insert(Radius {
                r: blueprint.radius,
            })
            .insert(Mass(blueprint.mass))
            .insert(Speed {
                speed: blueprint.movespeed,
                base: blueprint.movespeed,
            })
            .insert(Acceleration {
                acc: blueprint.acceleration,
                base: blueprint.acceleration,
            })
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
                multiplier: 1.0,
            })
            .insert(AvoidWallsBoid {
                avoidance_radius: 4.,
                multiplier: 6.,
                cell_size_multiplier: 0.55,
            })
            .insert(StoppingBoid { multiplier: 20. })
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
                is_overriding_velocity: false,
            })
            .insert(AppliedDamage {
                damages: Vec::new(),
            })
            .insert(effects::ResolveEffectsBuffer { vec: Vec::new() })
            .insert(effects::BuffHolder {
                set: std::collections::HashSet::new(),
            })
            .insert(Armor {
                armor: blueprint.armor,
                base: blueprint.armor,
            })
            .insert(MagicArmor {
                percent_resist: blueprint.magic_resist,
                base: blueprint.magic_resist,
            })
            .insert(HealEfficacy(1.0))
            .insert(AttackEnemyBehavior {})
            .insert(Hitpoints {
                max_hp: blueprint.hitpoints,
                hp: blueprint.hitpoints,
            })
            .insert(SpatialAwareness {
                radius: blueprint.awareness,
            })
            .id();

        // Insert Weapons
        let mut unit_actions = actions::UnitActions { vec: Vec::new() };
        for weapon in blueprint.weapons.iter() {
            if let Weapon::Melee(melee_weapon) = weapon {
                let melee_attack = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: melee_weapon.impact_time,
                            complete_time: melee_weapon.full_swing_time,
                            cooldown_time: melee_weapon.cooldown_time,
                        },
                        melee_weapon.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::normal_attack(),
                        "attack".to_string(),
                    ))
                    .insert(actions::Cleave {
                        angle_degrees: melee_weapon.cleave_degrees,
                    })
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::DamageEffect(DamageInstance {
                            damage: melee_weapon.damage,
                            delay: 0.0,
                            damage_type: DamageType::Normal,
                        })],
                    })
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(melee_attack);

                self.world.entity_mut(ent).insert(ChargeAtEnemyBoid {
                    target: None,
                    target_timer: 0.0,
                    charge_radius: melee_weapon.range * 3.,
                    multiplier: 1.,
                });
            } else if let Weapon::Projectile(projectile_weapon) = weapon {
                let projectile_attack = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: projectile_weapon.impact_time,
                            complete_time: projectile_weapon.full_swing_time,
                            cooldown_time: projectile_weapon.cooldown_time,
                        },
                        projectile_weapon.range,
                        actions::ImpactType::Projectile,
                        actions::TargetFlags::normal_attack(),
                        "attack".to_string(),
                    ))
                    .insert(projectiles::Splash {
                        radius: projectile_weapon.splash_radius,
                    })
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::DamageEffect(DamageInstance {
                            damage: projectile_weapon.damage,
                            delay: 0.0,
                            damage_type: DamageType::Normal,
                        })],
                    })
                    .insert(projectiles::ActionProjectileDetails {
                        projectile_speed: projectile_weapon.projectile_speed,
                        projectile_scale: projectile_weapon.projectile_scale,
                        projectile_texture: projectile_weapon.projectile_texture,
                        contact_distance: 12.0,
                    })
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(projectile_attack);

                self.world.entity_mut(ent).insert(KiteNearestEnemyBoid {
                    multiplier: 4.,
                    kite_radius: projectile_weapon.range,
                });
            }
        }

        for spell in blueprint.abilities.iter() {
            if let UnitAbility::Cleanse(cleanse) = spell {
                let cleanse_spell = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: cleanse.impact_time,
                            complete_time: cleanse.swing_time,
                            cooldown_time: cleanse.cooldown,
                        },
                        cleanse.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::cleanse(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(cleanse.effect_texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::CleanseEffect],
                    })
                    .id();
                unit_actions.vec.push(cleanse_spell);
            } else if let UnitAbility::SlowPoison(poison) = spell {
                // Unstable; depends on attack being the first action in unit list
                if let Some(main_attack) = unit_actions.vec.get_mut(0) {
                    if let Some(mut effects) = self
                        .world
                        .entity_mut(*main_attack)
                        .get_mut::<actions::OnHitEffects>()
                    {
                        effects.vec.push(effects::Effect::PoisonEffect(*poison));
                    }
                }
            } else if let UnitAbility::ArmorReduction(poison) = spell {
                // Unstable; depends on attack being the first action in unit list
                if let Some(main_attack) = unit_actions.vec.get_mut(0) {
                    if let Some(mut effects) = self
                        .world
                        .entity_mut(*main_attack)
                        .get_mut::<actions::OnHitEffects>()
                    {
                        effects
                            .vec
                            .push(effects::Effect::ArmorReductionEffect(*poison));
                    }
                }
            } else if let UnitAbility::Stun(stun) = spell {
                // Unstable; depends on attack being the first action in unit list
                if let Some(main_attack) = unit_actions.vec.get_mut(0) {
                    if let Some(mut effects) = self
                        .world
                        .entity_mut(*main_attack)
                        .get_mut::<actions::OnHitEffects>()
                    {
                        effects.vec.push(effects::Effect::StunEffect(*stun));
                    }
                }
            } else if let UnitAbility::Heal(heal) = spell {
                let heal_spell = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: heal.impact_time,
                            complete_time: heal.swing_time,
                            cooldown_time: heal.cooldown,
                        },
                        heal.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::heal(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(heal.effect_texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::HealEffect(heal.heal_amount)],
                    })
                    .id();
                unit_actions.vec.push(heal_spell);
            } else if let UnitAbility::BuffResistance(heal) = spell {
                let buff = effects::StatBuff {
                    armor_buff: 0.,
                    heal_efficacy_mult_buff: 0.,
                    acceleration_buff: 0.,
                    speed_buff: 0.,
                    magic_armor_buff: heal.magic_armor_amount,
                };
                let heal_spell = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: heal.impact_time,
                            complete_time: heal.swing_time,
                            cooldown_time: heal.cooldown,
                        },
                        heal.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::normal_buff(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(heal.effect_texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![
                            effects::Effect::ApplyStatBuffEffect(buff, heal.duration),
                            effects::Effect::Visual(effects::SpawnVisualEffect {
                                texture: heal.effect_texture,
                                duration: heal.duration,
                            }),
                        ],
                    })
                    .id();
                unit_actions.vec.push(heal_spell);
            } else if let UnitAbility::Fortify(heal) = spell {
                let buff = effects::StatBuff {
                    armor_buff: heal.armor_amount,
                    heal_efficacy_mult_buff: 0.,
                    acceleration_buff: 0.,
                    speed_buff: 0.,
                    magic_armor_buff: 0.,
                };
                let heal_spell = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: heal.impact_time,
                            complete_time: heal.swing_time,
                            cooldown_time: heal.cooldown,
                        },
                        heal.range,
                        actions::ImpactType::Projectile,
                        actions::TargetFlags::heal(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(heal.effect_texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![
                            effects::Effect::HealEffect(heal.heal_immediate),
                            effects::Effect::HealOverTimeEffect {
                                amount_per_second: heal.heal_over_time / heal.duration,
                                duration: heal.duration,
                            },
                            effects::Effect::ApplyStatBuffEffect(buff, heal.duration),
                            effects::Effect::Visual(effects::SpawnVisualEffect {
                                texture: heal.effect_texture,
                                duration: heal.duration,
                            }),
                        ],
                    })
                    .insert(projectiles::ActionProjectileDetails {
                        projectile_speed: 300.,
                        projectile_scale: 0.6,
                        projectile_texture: heal.effect_texture,
                        contact_distance: 12.0,
                    })
                    .id();
                unit_actions.vec.push(heal_spell);
            } else if let UnitAbility::MagicMissile(missile) = spell {
                let missile_attack = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: missile.impact_time,
                            complete_time: missile.swing_time,
                            cooldown_time: missile.cooldown,
                        },
                        missile.range,
                        actions::ImpactType::Projectile,
                        actions::TargetFlags::normal_attack(),
                        "cast".to_string(),
                    ))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::DamageEffect(DamageInstance {
                            damage: missile.damage,
                            delay: 0.0,
                            damage_type: DamageType::Magic,
                        })],
                    })
                    .insert(projectiles::ActionProjectileDetails {
                        projectile_speed: 175.,
                        projectile_scale: 0.6,
                        projectile_texture: missile.effect_texture,
                        contact_distance: 12.0,
                    })
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(missile_attack);
            } else if let UnitAbility::Whirlwind(missile) = spell {
                let whirlwind = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: missile.impact_time,
                            complete_time: missile.swing_time,
                            cooldown_time: missile.cooldown,
                        },
                        missile.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::normal_attack(),
                        "cast".to_string(),
                    ))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::DamageEffect(DamageInstance {
                            damage: missile.damage,
                            delay: 0.0,
                            damage_type: DamageType::Magic,
                        })],
                    })
                    .insert(actions::Cleave {
                        angle_degrees: 360.,
                    })
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(whirlwind);
            } else if let UnitAbility::Overdrive(overdrive) = spell {
                let heal_spell = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: overdrive.impact_time,
                            complete_time: overdrive.swing_time,
                            cooldown_time: overdrive.cooldown,
                        },
                        overdrive.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::normal_buff(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(overdrive.effect_texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::OverdriveEffect(*overdrive)],
                    })
                    .id();
                unit_actions.vec.push(heal_spell);
            } else if let UnitAbility::Confusion(confusion) = spell {
                if let Some(main_attack) = unit_actions.vec.get_mut(0) {
                    if let Some(mut effects) = self
                        .world
                        .entity_mut(*main_attack)
                        .get_mut::<actions::OnHitEffects>()
                    {
                        effects
                            .vec
                            .push(effects::Effect::ConfusionEffect(*confusion));
                    }
                }
            } else if let UnitAbility::AntiHeal(antiheal) = spell {
                if let Some(main_attack) = unit_actions.vec.get_mut(0) {
                    if let Some(mut effects) = self
                        .world
                        .entity_mut(*main_attack)
                        .get_mut::<actions::OnHitEffects>()
                    {
                        effects.vec.push(effects::Effect::AntiHeal(*antiheal));
                    }
                }
            } else if let UnitAbility::Backstab(backstab) = spell {
                let whirlwind = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: backstab.impact_time,
                            complete_time: backstab.swing_time,
                            cooldown_time: backstab.cooldown,
                        },
                        backstab.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::furthest_enemy(),
                        "cast".to_string(),
                    ))
                    .insert(actions::OnHitEffects {
                        vec: vec![
                            effects::Effect::DamageEffect(DamageInstance {
                                damage: backstab.damage,
                                delay: 0.0,
                                damage_type: DamageType::Normal,
                            }),
                            effects::Effect::TeleportBehindTargetEffect(ent),
                            effects::Effect::Visual(effects::SpawnVisualEffect {
                                texture: backstab.texture,
                                duration: 1.0,
                            }),
                        ],
                    })
                    .insert(actions::Cooldown(backstab.cooldown))
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(whirlwind);
            } else if let UnitAbility::DamageBuff(buff) = spell {
                let whirlwind = self
                    .world
                    .spawn()
                    .insert_bundle(actions::ActionBundle::new(
                        actions::SwingDetails {
                            impact_time: buff.impact_time,
                            complete_time: buff.swing_time,
                            cooldown_time: buff.cooldown,
                        },
                        buff.range,
                        actions::ImpactType::Instant,
                        actions::TargetFlags::normal_buff(),
                        "cast".to_string(),
                    ))
                    .insert(actions::EffectTexture(buff.texture))
                    .insert(actions::OnHitEffects {
                        vec: vec![effects::Effect::DamageBuffEffect(*buff)],
                    })
                    .id();

                // Add weapon to unit actions
                unit_actions.vec.push(whirlwind);
            }
        }

        // Give unit action set
        self.world.entity_mut(ent).insert(unit_actions);
        return ent.id();
    }

    #[method]
    #[profiled]
    fn _physics_process(&mut self, #[base] base: &Node2D, delta: f32) {
        if !self.running {
            return;
        }

        self.world.insert_resource(DeltaPhysics { seconds: delta });
        self.world.insert_resource(Clock(self.clock));
        self.world.insert_resource(self.terrain_map.clone());
        self.clock += 1;
        self.schedule_logic.run(&mut self.world);
        self.update_victor();
    }

    fn update_victor(&mut self) {
        let mut victor: i32 = -1;
        let mut living_teams = std::collections::HashSet::<TeamValue>::new();
        for alignment in self.world.query::<&TeamAlignment>().iter(&self.world) {
            living_teams.insert(alignment.alignment);
            if let TeamValue::Team(intval) = alignment.alignment {
                victor = intval as i32;
            }
        }

        // If only one remaining player
        if living_teams.len() < 2 {
            self.victor = victor;
        }
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

    fn _process_event_signal_queue(&mut self, base: &Node2D) {
        if let Some(queue) = self.world.get_resource::<EventQueue>() {
            for event in queue.0.iter() {
                match event {
                    EventCue::Audio(cue) => {
                        let variant_arr = VariantArray::new();
                        variant_arr.push(cue.texture);
                        variant_arr.push(cue.event.clone());
                        variant_arr.push(cue.location);
                        base.emit_signal("event_cue", &[variant_arr.into_shared().to_variant()]);
                    }
                    EventCue::Damage(cue) => {
                        let variant_arr = VariantArray::new();
                        variant_arr.push(cue.damage);
                        variant_arr.push(cue.damage_type.clone());
                        variant_arr.push(cue.location);
                        base.emit_signal("damage_cue", &[variant_arr.into_shared().to_variant()]);
                    }
                }
            }
        }
        self.world.insert_resource(EventQueue(Vec::new()));
    }

    #[method]
    fn _process_in_place(&mut self) {
        self.world.insert_resource(Delta { seconds: 0f32 });
        self._process_new_canvas_items();
        self.schedule.run(&mut self.world);
    }

    #[method]
    #[profiled]
    fn _process(&mut self, #[base] base: &Node2D, delta: f32) {
        if !self.running {
            return;
        }
        self.world.insert_resource(Delta { seconds: delta });

        self._process_new_canvas_items();
        self._process_event_signal_queue(base);
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

    #[method]
    fn _init(&mut self) {}
}

fn init(handle: InitHandle) {
    handle.add_class::<ECSWorld>();
}

godot_init!(init);
