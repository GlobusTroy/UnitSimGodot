use gdnative::prelude::*;
use std::collections::HashMap;

use crate::{physics::spatial_structures::*, unit::TeamValue, ECSWorld};

pub fn draw_terrain_map(world: &mut ECSWorld, base: TRef<Node2D>) {
    let terrain_map = world.world.get_resource::<TerrainMap>();

    if let Some(map) = terrain_map {
        let label = Label::new();
        let font = label.get_font("", "");

        let rect_size = Vector2 {
            x: map.cell_size,
            y: map.cell_size,
        };
        let half_size = rect_size / 2.;
        for x in 0..map.max_bounds.0 {
            for y in 0..map.max_bounds.1 {
                let rect_pos = Vector2 {
                    x: x as f32 * map.cell_size,
                    y: y as f32 * map.cell_size,
                };
                base.draw_rect(
                    Rect2 {
                        position: rect_pos,
                        size: rect_size,
                    },
                    Color {
                        r: 0.,
                        g: 1.,
                        b: 1.,
                        a: 1.,
                    },
                    false,
                    2f64,
                    false,
                );
            }
        }

        for (cell, _terrain) in map.map.iter() {
            let rect_pos = Vector2 {
                x: map.cell_size * 0.125 + cell.0 as f32 * map.cell_size,
                y: map.cell_size * 0.125 + cell.1 as f32 * map.cell_size,
            };
            base.draw_rect(
                Rect2 {
                    position: rect_pos,
                    size: rect_size * 0.75,
                },
                Color {
                    r: 1.,
                    g: 0.,
                    b: 0.,
                    a: 1.,
                },
                true,
                0.,
                false,
            );
        }
    }
}

pub fn draw_integration_values(world: &mut ECSWorld, base: TRef<Node2D>) {
    let terrain_map = world.world.get_resource::<TerrainMap>();
    let integration_map = world
        .world
        .get_resource::<HashMap<SpatialHashCell, HashMap<TeamValue, f32>>>();
    let label = Label::new();
    let font = label.get_font("", "");

    if let Some(map) = terrain_map {
        if let Some(integration_map) = integration_map {
            let rect_size = Vector2 {
                x: map.cell_size,
                y: map.cell_size,
            };
            let half_y = Vector2 {
                x: 0.,
                y: map.cell_size / 2.,
            };
            for (cell, integration) in integration_map.iter() {
                let mut text: String = String::new();
                for (team, val) in integration.iter() {
                    if let TeamValue::Team(integer) = team {
                        if integer < &2 {
                            text.push_str(&format!("{},", *val as i32));
                        }
                    }
                }
                let rect_pos = Vector2 {
                    x: cell.0 as f32 * map.cell_size,
                    y: cell.1 as f32 * map.cell_size,
                };
                if let Some(ref font_val) = font {
                    base.draw_string(
                        font_val,
                        rect_pos + half_y,
                        format!("{}", text),
                        Color {
                            r: 1.,
                            g: 0.,
                            b: 0.,
                            a: 1.,
                        },
                        map.cell_size as i64,
                    );
                }
            }
        }
    }
}

pub fn draw_flow_field(world: &mut ECSWorld, base: TRef<Node2D>) {
    let flow_field_map = world.world.get_resource::<FlowFieldsTowardsEnemies>();
    if let Some(flow_field) = flow_field_map {
        let half_size = Vector2::ONE * (flow_field.cell_size / 2.);
        for (cell, vector_map) in flow_field.map.iter() {
            let rect_pos = Vector2 {
                x: cell.0 as f32 * flow_field.cell_size,
                y: cell.1 as f32 * flow_field.cell_size,
            };
            for (team, val) in vector_map.iter() {
                if let TeamValue::Team(integer) = team {
                    if integer < &2 {
                        base.draw_line(
                            rect_pos + half_size,
                            rect_pos + half_size + (*val * 16.),
                            Color {
                                r: 1.,
                                g: 0.,
                                b: 1.,
                                a: 1.,
                            },
                            4.,
                            false,
                        );
                    }
                }
            }
        }
    }
}
