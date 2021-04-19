use std::collections::HashMap;
use self_compare::SliceCompareExt;

use rand::Rng;

use bevy::{
    prelude::*,
    tasks::{ComputeTaskPool},
    input::system::exit_on_esc_system,
    render::pass::ClearColor,
    sprite::collide_aabb::{collide, Collision},
};

fn main() {
    App::build()
        .insert_resource(ClearColor(Color::rgb(0.24, 0.5, 0.01)))
        .insert_resource(WindowDescriptor {
            title: "Sidewars".to_owned(),
            .. Default::default()
        })
        .insert_resource(MouseLoc(Default::default()))
        .add_plugins(DefaultPlugins)
        .init_resource::<Materials>()
        .add_startup_system(setup.system())
        .add_system(collision_system.system())
        .add_system(fighter_movement.system())
        .add_system(fighter_health_bar_system.system())
        .add_system(exit_on_esc_system.system())
        .add_system(scoreboard_text_system.system())
        .add_system(fighting_system.system())
        .add_system(mouse_location_system.system())
        .add_system(soldier_placement_system.system())
        .add_system(timeout_system.system())
        .run();
}

type Level = u8;

#[derive(Debug, Clone, Copy)]
struct Skills {
    attack: Level,
    defence: Level,
    strength: Level,
    // ranged: Level,
    hp: Level,
    speed: Level,
}

#[derive(Debug, Clone, Copy)]
struct Fighter {
    skills: Skills,
    // MAYBE: gear (that gives bonuses in each)
    hp: u8,
    protection: u8,
    fighting: Option<Entity>,
    attack_cooldown: f32,
    waiting: bool,
}

impl Fighter {
    pub fn new(skills: Skills) -> Self {
        Fighter {
            hp: skills.hp,
            protection: 0,
            skills,
            fighting: None,
            attack_cooldown: 0.,
            waiting: false,
        }
    }
    pub fn with_protection(skills: Skills, protection: u8) -> Self {
        Fighter {
            protection,
            .. Fighter::new(skills)
        }
    }
    fn moving(&self) -> bool {
        !self.waiting && self.fighting.is_none()
    }
}

struct HealthBar;

struct Timeout {
    time_left: f32,
    tied_to: Vec<Entity>,
}

impl Timeout {
    const fn new(time_left: f32) -> Self {
        Timeout {
            time_left,
            tied_to: Vec::new(),
        }
    }
    fn tied_to(self, tied_to: Vec<Entity>) -> Self {
        Timeout {
            tied_to,
            .. self
        }
    }
}

fn fighter_sprite_bundle(x: f32, y: f32, flipped: bool, materials: &Materials) -> SpriteBundle {
    let mut transform = Transform::from_translation(Vec3::new(x, y, 0.0));
    if flipped {
        transform.scale.x = -transform.scale.x;
    }
    SpriteBundle {
        material: materials.fighter.clone(),
        transform,
        sprite: Sprite::new(Vec2::new(32., 32.)),
        .. Default::default()
    }
}

fn spawn_fighter(cmds: &mut Commands, x: f32, y: f32, flipped: bool, materials: &Materials, skills: Skills) {
    cmds.spawn()
        .insert_bundle(fighter_sprite_bundle(x, y, flipped, materials))
        .insert(Fighter::new(skills))
        .with_children(|parent| {
            parent
                .spawn_bundle(SpriteBundle {
                    material: materials.black.clone(),
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite::new(Vec2::new(34.0, 10.0)),
                    ..Default::default()
                });
            parent.spawn_bundle(SpriteBundle {
                    material: materials.green.clone(),
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite::new(Vec2::new(32.0, 8.0)),
                    ..Default::default()
                })
                .insert(HealthBar);
        });
}

fn fighter_health_bar_system(
    query: Query<(&Fighter, &Children)>,
    mut health_query: Query<(&mut Transform, &mut Sprite), With<HealthBar>>,
) {
    for (fighter, children) in query.iter() {
        for child in &**children {
            if let Ok((mut trans, mut spr)) = health_query.get_mut(child.clone()) {
                let x = 32. * fighter.hp as f32 / fighter.skills.hp as f32;
                spr.size.x = x;
                trans.translation.x = 0.5 * x - 16.;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Materials {
    font: Handle<Font>,
    fighter: Handle<ColorMaterial>,
    black: Handle<ColorMaterial>,
    green: Handle<ColorMaterial>,
    red: Handle<ColorMaterial>,
}

impl FromWorld for Materials {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();
        let font = asset_server.load("DroidSansMono.ttf");
        let fighter_asset = asset_server.load("fighter.png").into();

        let mut materials = world.get_resource_mut::<Assets<ColorMaterial>>().unwrap();
        Self {
            font,
            fighter: materials.add(fighter_asset),
            black: materials.add(Color::rgba(0., 0., 0., 0.33).into()),
            green: materials.add(Color::rgba(0., 1., 0., 0.33).into()),
            red: materials.add(Color::rgb(1., 0., 0.).into()),
        }
    }
}

fn setup(
    mut commands: Commands,
    materials: Res<Materials>,
) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(UiCameraBundle::default());
    commands.spawn_bundle(TextBundle {
        text: Text {
            sections: vec![
                TextSection {
                    value: "Score: ".to_string(),
                    style: TextStyle {
                        font: materials.font.clone(),
                        color: Color::rgb(0.5, 0.5, 1.0),
                        font_size: 40.0,
                    }
                },
                TextSection {
                    value: "".to_string(),
                    style: TextStyle {
                        font: materials.font.clone(),
                        color: Color::rgb(0.5, 0.5, 1.0),
                        font_size: 40.0,
                    }
                }
            ],
            .. Default::default()
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }).insert(Scoreboard { score: 0 });
}

#[derive(Debug, Default, Copy, Clone)]
pub struct MouseLoc(Vec2);

fn mouse_location_system(
    mut ev_cursor: EventReader<CursorMoved>,
    windows: Res<Windows>,
    mut mouse_loc: ResMut<MouseLoc>,
) {
    let window = windows.get_primary().unwrap();
    let primary_id = window.id();
    let w = window.width() / 2.;
    let h = window.height() / 2.;
    for cm in ev_cursor.iter().filter(|cm| cm.id == primary_id).last() {
        mouse_loc.0 = cm.position - Vec2::new(-w, -h);
    }
}


fn fighter_movement(
    pool: Res<ComputeTaskPool>,
    time: Res<Time>,
    windows: Res<Windows>,
    mut query: Query<(&mut Transform, &Fighter)>,
) {
    let window = windows.get_primary().expect("No primary window.");
    let width = window.width();
    let height = window.height();

    let delta = time.delta_seconds();

    query.par_for_each_mut(&pool, 32, |(mut transform, fighter)| {
        if !fighter.moving() {
            return
        }

        let scale_x = transform.scale.x;
        let translation = &mut transform.translation;

        translation.x += 3. * scale_x * fighter.skills.speed as f32 * delta;

        // Messy code to keep inside frame
        *translation += Vec3::new(width * 1.5, height * 1.5, 0.);
        translation.x %= width;
        translation.y %= height;
        *translation -= Vec3::new(width * 0.5, height * 0.5, 0.);
    })
}

#[derive(Debug)]
struct Scoreboard {
    score: i32,
}

fn scoreboard_text_system(mut query: Query<(&mut Text, &Scoreboard)>) {
    for (mut text, scoreboard) in query.iter_mut() {
        text.sections[1].value = format!("{}", scoreboard.score);
    }
}

fn collision_system(
    mut query: Query<(Entity, &mut Fighter, &Transform, &Sprite)>,
) {
    let mut waiting = HashMap::new();
    let mut ents: Vec<_> = query.iter_mut().collect();
    
    ents.compare_self_mut(|
        (left_entity, left_fighter, left_trans, left_spr),
        (right_entity, right_fighter, right_trans, right_spr)
    | {
        let waiting = &mut waiting;
        let collision = collide(
            left_trans.translation,
            left_spr.size,
            right_trans.translation,
            right_spr.size,
        );
        if let Some(collision) = collision {
            if left_trans.scale.x == right_trans.scale.x {
                let ((left_entity, right_entity), (left_fighter, right_fighter)) = if left_trans.scale.x > 0. {
                    ((left_entity, right_entity), (left_fighter, right_fighter))
                } else {
                    ((right_entity, left_entity), (right_fighter, left_fighter))
                };

                match collision {
                    Collision::Left | Collision::Top => {
                        left_fighter.waiting = true;
                        waiting.insert(left_entity.clone(), true);
                    }
                    Collision::Right | Collision::Bottom => {
                        right_fighter.waiting = true;
                        waiting.insert(right_entity.clone(), true);
                    }
                }
            } else {
                left_fighter.fighting = Some(right_entity.clone());
                right_fighter.fighting = Some(left_entity.clone());
            }
        } else {
            if left_fighter.waiting {
                waiting.entry(left_entity.clone()).or_insert(false);
            }
            if right_fighter.waiting {
                waiting.entry(right_entity.clone()).or_insert(false);
            }
        }
    });

    for (ent, v) in waiting.into_iter().filter(|(_, v)| !v) {
        query.get_mut(ent).unwrap().1.waiting = v;
    }
}

use std::sync::mpsc::sync_channel;

const COOLDOWN: f32 = 1.;

fn fighting_system(
    mut commands: Commands,
    pool: Res<ComputeTaskPool>,
    time: Res<Time>,
    materials: Res<Materials>,
    mut query: Query<(Entity, &mut Fighter, &Transform)>
) {
    let (tx, rx) = sync_channel(query.iter_mut().len());

    let delta = time.delta_seconds();

    query
        .par_for_each_mut(&pool, 32, move |(ent, mut fighter, _)| {
            fighter.attack_cooldown -= delta;
            if fighter.attack_cooldown <= 0. {
                fighter.attack_cooldown = 0.;
                if let Some(fighting) = fighter.fighting {
                    tx.send((ent, fighting, fighter.skills)).unwrap();
                }
            }
        });

    let mut rng = rand::thread_rng();

    for (fighter, fought_ent, skills) in rx.into_iter() {
        if let Ok((_, mut fought, f_trans)) = query.get_mut(fought_ent) {
            if rng.gen_range(0..=skills.attack) > rng.gen_range(0..=fought.skills.defence) {
                let dmg = rng.gen_range(1..=skills.strength);

                let actual_dmg = dmg.saturating_sub(rng.gen_range(0..=fought.protection));

                fought.hp = fought.hp.saturating_sub(actual_dmg);

                let mut transform = Transform::from_translation(f_trans.translation);

                transform.translation.y += 45.;

                let ent = commands.spawn_bundle(TextBundle {
                    text: Text {
                        sections: vec![
                            TextSection {
                                value: format!("{}", actual_dmg),
                                style: TextStyle {
                                    font: materials.font.clone(),
                                    color: Color::rgb(0., 0., 0.),
                                    font_size: 16.,
                                    .. Default::default()
                                }
                            }
                        ],
                        .. Default::default()
                    },
                    style: Style {
                        position_type: PositionType::Absolute,
                        position: Rect {
                            left: Val::Px(transform.translation.x),
                            top: Val::Px(transform.translation.y),
                            .. Default::default()
                        },
                        .. Default::default()
                    },
                    .. Default::default()
                }).id();
                commands.spawn_bundle(SpriteBundle {
                    transform,
                    material: materials.red.clone(),
                    sprite: Sprite::new(Vec2::new(10., 10.)),
                    ..Default::default()
                }).insert(Timeout::new(1.2).tied_to(vec![ent]));

                if fought.hp <= 0 {
                    commands.entity(fought_ent).despawn_recursive();
                }
            }
        } else {
            let (_, mut fighter, _) = query.get_mut(fighter).unwrap();
            fighter.fighting = None;
        }
        let (_, mut fighter, _) = query.get_mut(fighter).unwrap();
        fighter.attack_cooldown += COOLDOWN;
    }
}

fn soldier_placement_system(
    mut commands: Commands,
    mouse_loc: Res<MouseLoc>,
    materials: Res<Materials>,
    mouse_button: Res<Input<MouseButton>>,
) {
    for button in mouse_button.get_just_pressed() {
        let flipped;
        match button {
            MouseButton::Right => flipped = false,
            MouseButton::Left => flipped = true,
            MouseButton::Middle => {
                eprintln!("{:?}", mouse_loc.0);
                continue
            }
            _ => continue,
        }

        spawn_fighter(&mut commands, mouse_loc.0.x, mouse_loc.0.y, flipped, &materials, Skills {
            attack: 30,
            defence: 1,
            hp: 20,
            strength: 5,
            speed: 35,
        });
    }
}

fn timeout_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Timeout)>
) {
    for (ent, mut timeout) in query.iter_mut() {
        let time = time.delta_seconds();
        timeout.time_left -= time;
        if timeout.time_left <= 0. {
            commands.entity(ent).despawn();
            for &ent in &timeout.tied_to {
                commands.entity(ent).despawn();
            }
        }
    }
}