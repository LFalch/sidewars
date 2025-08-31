use std::collections::HashMap;
use self_compare::SliceCompareExt;

use rand::{seq::SliceRandom, Rng};

use bevy::{
    prelude::*,
    render::camera::Camera,
    sprite::collide_aabb::{collide, Collision},
    app::AppExit, window::PrimaryWindow,
};

pub fn exit_on_esc_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut exit: EventWriter<AppExit>,
) {
    if keyboard_input.pressed(KeyCode::LShift) && keyboard_input.just_pressed(KeyCode::Escape) {
        exit.send(AppExit);
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::rgb(0.24, 0.5, 0.01)))
        .insert_resource(MouseLoc(Default::default()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sidewars".to_owned(),
                .. default()
            }),
            .. default()
        }))
        .init_resource::<Materials>()
        .add_startup_system(setup)
        .add_system(collision_system)
        .add_system(fighter_movement)
        .add_system(figter_siege)
        .add_system(fighter_health_bar_system)
        .add_system(exit_on_esc_system)
        .add_system(scoreboard_text_system)
        .add_system(fighting_system)
        .add_system(mouse_location_system)
        .add_system(soldier_placement_system)
        .add_system(timeout_system)
        .run();
}

type Level = u8;

#[derive(Debug, Clone, Copy)]
struct Skills {
    price: u8,
    attack: Level,
    defence: Level,
    strength: Level,
    // ranged: Level,
    hp: Level,
    speed: Level,
    siege: Level,
}
impl Skills {
    const PRIVATE: Self = Self {
        price: 2,
        attack: 15,
        defence: 15,
        hp: 20,
        strength: 5,
        speed: 30,
        siege: 5,
    };
    const FIGHTER: Self = Self {
        price: 3,
        attack: 30,
        defence: 5,
        hp: 15,
        strength: 10,
        speed: 35,
        siege: 7,
    };
    const SHIELDSMAN: Self = Self {
        price: 3,
        attack: 5,
        defence: 30,
        hp: 30,
        strength: 5,
        speed: 20,
        siege: 1,
    };
}

#[derive(Debug, Clone, Copy, Component)]
struct Fighter {
    skills: Skills,
    hp: u8,
    protection: u8, fighting: Option<Entity>,
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
    fn moving(&self) -> bool {
        !self.waiting && self.fighting.is_none()
    }
}

#[derive(Component)]
struct HealthBar;

#[derive(Debug, Clone, Component)]
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
        texture: materials.fighter.clone(),
        transform,
        sprite: Sprite {
            custom_size: Some(Vec2::new(32.0, 32.0)),
            .. default()
        },
        .. Default::default()
    }
}

fn spawn_fighter(cmds: &mut Commands, x: f32, y: f32, flipped: bool, materials: &Materials, skills: Skills) {
    cmds
        .spawn(fighter_sprite_bundle(x, y, flipped, materials))
        .insert(Fighter::new(skills))
        .with_children(|parent| {
            parent
                .spawn(SpriteBundle {
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite {
                        color: materials.black,
                        custom_size: Some(Vec2::new(34.0, 10.0)), .. default()
                    },
                    ..Default::default()
                });
            parent.spawn(SpriteBundle {
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite {
                        color: materials.green,
                        custom_size: Some(Vec2::new(32.0, 8.0)), .. default() },
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
                spr.custom_size.as_mut().unwrap().x = x;
                trans.translation.x = 0.5 * x - 16.;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
struct SpawnZone {
    x: f32,
    timer: f32,
    height: f32,
}
#[derive(Debug, Clone, Copy, Resource)]
struct Money {
    left: i16,
    right: i16,
}

#[derive(Debug, Clone)]
#[derive(Resource)]
struct Materials {
    font: Handle<Font>,
    fighter: Handle<Image>,
    black: Color,
    green: Color,
    yellow: Color,
    red: Color,
}

impl FromWorld for Materials {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();
        let font = asset_server.load("DroidSansMono.ttf");
        let fighter_asset = asset_server.load("fighter.png");

        Self {
            font,
            fighter: fighter_asset,
            black: Color::rgba(0., 0., 0., 0.33),
            green: Color::rgba(0., 1., 0., 0.33),
            yellow: Color::rgba(1., 1., 0., 0.33),
            red: Color::rgb(1., 0., 0.),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(Component)]
struct MainCamera;

const SPAWN_WIDTH: f32 = 64.;
fn setup(
    mut commands: Commands,
    window_query: Query<&Window, With<PrimaryWindow>>,
    materials: Res<Materials>,
) {
    let window = window_query.get_single().expect("No primary window.");
    let height = window.height();
    let width = window.width();

    commands.spawn(Camera2dBundle::default()).insert(MainCamera);
    commands.spawn(TextBundle {
        text: Text {
            sections: vec![
                TextSection {
                    value: "Attack: ¤".to_string(),
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
                },
                TextSection {
                    value: "\nDefender: ¤".to_string(),
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
            position: UiRect {
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }).insert(Scoreboard);
    let zone_x = width/2.-SPAWN_WIDTH/2.;
    commands.spawn(SpriteBundle {
        transform: Transform::from_xyz(-zone_x, 0., 0.),
        sprite: Sprite {
            color: materials.yellow.with_a(1.),
            custom_size: Some(Vec2::new(SPAWN_WIDTH, height)),
            .. default()
        },
        .. default()
    });
    commands.spawn(SpriteBundle {
        transform: Transform::from_xyz(zone_x, 0., 0.),
        sprite: Sprite {
            color: materials.yellow.with_a(1.),
            custom_size: Some(Vec2::new(SPAWN_WIDTH, height)),
            .. default()
        },
        .. default()
    });
    commands.insert_resource(SpawnZone { x: zone_x, timer: 1., height, });
    commands.insert_resource(Money { left: 30, right: 25, });
}

#[derive(Debug, Default, Copy, Clone)]
#[derive(Resource)]
pub struct MouseLoc(Vec2);

fn mouse_location_system(
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut mouse_loc: ResMut<MouseLoc>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>
) {
    let (camera, camera_transform) = camera_q.single();

    let window = window_query.single();

    mouse_loc.0 = window.cursor_position().unwrap_or(Vec2::ZERO);
    mouse_loc.0 = camera.viewport_to_world_2d(camera_transform, mouse_loc.0).unwrap();
}


fn fighter_movement(
    time: Res<Time>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Transform, &Fighter)>,
) {
    let window = window_query.get_single().expect("No primary window.");
    let height = window.height();

    let delta = time.delta_seconds();

    query.par_iter_mut().for_each_mut(|(mut transform, fighter)| {
        if !fighter.moving() {
            return
        }

        let scale_x = transform.scale.x;
        let translation = &mut transform.translation;

        translation.x += 3. * scale_x * fighter.skills.speed as f32 * delta;

        // Messy code to keep inside frame
        translation.y += height * 1.5;
        translation.y %= height;
        translation.y -= height * 0.5;
    })
}

fn figter_siege(
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    query: Query<(Entity, &Transform, &Fighter)>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut money: ResMut<Money>,
) {
    let window = window_query.get_single().expect("No primary window.");
    let width = window.width();

    let (camera, global_transform) = camera_q.single();

    for (ent, transform, fighter) in query.iter() {
        let pos = camera.world_to_viewport(global_transform, transform.translation).unwrap();
        if pos.x > width {
            commands.entity(ent).despawn_recursive();
            money.left += fighter.skills.siege as i16;
        } else if pos.x < 0. {
            commands.entity(ent).despawn_recursive();
            money.right += fighter.skills.siege as i16;
        }
    }
}

#[derive(Debug, Component)]
struct Scoreboard;

fn scoreboard_text_system(mut query: Query<(&mut Text, &Scoreboard)>, money: Res<Money>) {
    for (mut text, _) in query.iter_mut() {
        text.sections[1].value = format!("{}", money.left);
        text.sections[3].value = format!("{}", money.right);
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
            left_spr.custom_size.unwrap(),
            right_trans.translation,
            right_spr.custom_size.unwrap(),
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
                    Collision::Right | Collision::Bottom | Collision::Inside => {
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
    time: Res<Time>,
    materials: Res<Materials>,
    mut money: ResMut<Money>,
    mut query: Query<(Entity, &mut Fighter, &Transform)>
) {
    let (tx, rx) = sync_channel(query.iter_mut().len());

    let delta = time.delta_seconds();

    query
        .par_iter_mut().for_each_mut(move |(ent, mut fighter, _)| {
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
                transform.translation.z += 1.;

                let ent = commands.spawn(Text2dBundle {
                    text: Text {
                        sections: vec![
                            TextSection {
                                value: format!("{}", actual_dmg),
                                style: TextStyle {
                                    font: materials.font.clone(),
                                    font_size: 20.,
                                    color: Color::rgb(0., 0., 0.),
                                }
                            }
                        ],
                        .. Default::default()
                    },
                    transform: transform.clone()*Transform::from_translation(Vec3::new(0., 0., 2.)),
                    .. Default::default()
                }).id();
                commands.spawn(SpriteBundle {
                    transform,
                    sprite: Sprite {
                        color: materials.red,
                        custom_size: Some(Vec2::new(15., 15.)),
                        .. default()
                    },
                    .. default()
                }).insert(Timeout::new(1.15).tied_to(vec![ent]));

                if fought.hp <= 0 {
                    if f_trans.scale.x > 0. {
                        money.left += 1;
                    } else {
                        money.right += 1;
                    }
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
    mut spawn_zone: ResMut<SpawnZone>,
    mut money: ResMut<Money>,
    time: Res<Time>,
    mouse_button: Res<Input<MouseButton>>,
) {
    let location = mouse_loc.0;
    if location.x < -spawn_zone.x + SPAWN_WIDTH / 2. && money.left >= 1 {
        for button in mouse_button.get_just_pressed() {
            let skills = match button {
                MouseButton::Left => Skills::FIGHTER,
                MouseButton::Middle => Skills::PRIVATE,
                MouseButton::Right => Skills::SHIELDSMAN,
                _ => continue,
            };

            money.left -= skills.price as i16;
            spawn_fighter(&mut commands, -spawn_zone.x, location.y, false, &materials, skills);
        }
    }

    spawn_zone.timer -= time.delta_seconds();

    while spawn_zone.timer < 0. && money.right >= 1 {
        let denominator = (money.right / 7).max(1) as f32;
        spawn_zone.timer += 1. / denominator;
        let mut rng = rand::thread_rng();
        let y = rng.gen_range(-spawn_zone.height/2. .. spawn_zone.height/2.);
        let skills = *[
            Skills::FIGHTER, Skills::PRIVATE, Skills::SHIELDSMAN,
        ].choose(&mut rng).unwrap();

        money.right -= skills.price as i16;
        spawn_fighter(&mut commands, spawn_zone.x, y, true, &materials, skills);
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
