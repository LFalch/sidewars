use std::collections::HashMap;
use self_compare::SliceCompareExt;

use rand::Rng;

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
    attack: Level,
    defence: Level,
    strength: Level,
    // ranged: Level,
    hp: Level,
    speed: Level,
    siege: Level,
}

#[derive(Debug, Clone, Copy, Component)]
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

#[derive(Debug, Clone)]
#[derive(Resource)]
struct Materials {
    font: Handle<Font>,
    fighter: Handle<Image>,
    black: Color,
    green: Color,
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
            red: Color::rgb(1., 0., 0.),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[derive(Component)]
struct MainCamera;

fn setup(
    mut commands: Commands,
    materials: Res<Materials>,
) {
    commands.spawn(Camera2dBundle::default()).insert(MainCamera);
    commands.spawn(TextBundle {
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
            position: UiRect {
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
    mut scoreboard_query: Query<&mut Scoreboard>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    let window = window_query.get_single().expect("No primary window.");
    let width = window.width();

    let (camera, global_transform) = camera_q.single();

    for (ent, transform, fighter) in query.iter() {
        let pos = camera.world_to_viewport(global_transform, transform.translation).unwrap();
        if pos.x > width {
            commands.entity(ent).despawn_recursive();
            scoreboard_query.for_each_mut(|mut s| s.score += fighter.skills.siege as i32);
        } else if pos.x < 0. {
            commands.entity(ent).despawn_recursive();
            scoreboard_query.for_each_mut(|mut s| s.score -= fighter.skills.siege as i32);
        }
    }
}

#[derive(Debug, Component)]
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
            siege: 5,
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