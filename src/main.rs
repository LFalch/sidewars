use std::collections::HashMap;
use self_compare::SliceCompareExt;

use rand::prelude::*;

use bevy::{
    app::AppExit, math::bounding::{Aabb2d, IntersectsVolume}, prelude::*, render::camera::Camera, sprite::Anchor, window::PrimaryWindow
};

pub fn exit_on_esc_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
) {
    if keyboard_input.pressed(KeyCode::ShiftLeft) && keyboard_input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.24, 0.5, 0.01)))
        .insert_resource(MouseLoc(Default::default()))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sidewars".to_owned(),
                .. default()
            }),
            .. default()
        }))
        .init_resource::<Materials>()
        .add_systems(Startup,  setup)
        .add_systems(Update, collision_system)
        .add_systems(Update, fighter_movement)
        .add_systems(Update, figter_siege)
        .add_systems(Update, fighter_health_bar_system)
        .add_systems(Update, exit_on_esc_system)
        .add_systems(Update, scoreboard_text_system)
        .add_systems(Update, fighting_system)
        .add_systems(Update, mouse_location_system)
        .add_systems(Update, soldier_placement_system)
        .add_systems(Update, timeout_system)
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

fn fighter_sprite_bundle(x: f32, y: f32, flipped: bool, materials: &Materials) -> (Transform, Sprite) {
    (Transform::from_xyz(x, y, 0.), Sprite {
        image: materials.fighter.clone(),
        flip_x: flipped,
        anchor: Anchor::Center,
        custom_size: Some(Vec2::new(32., 32.)),
        .. Default::default()
    })
}

fn spawn_fighter(cmds: &mut Commands, x: f32, y: f32, flipped: bool, materials: &Materials, skills: Skills) {
    cmds
        .spawn(fighter_sprite_bundle(x, y, flipped, materials))
        .insert(Fighter::new(skills))
        .with_children(|parent| {
            parent
                .spawn((Transform::from_translation(Vec3::new(0., 30., 1.)),
                    Sprite {
                        color: materials.black,
                        custom_size: Some(Vec2::new(34.0, 10.0)),
                        .. default()
                    }));
            parent.spawn((Transform::from_translation(Vec3::new(0., 30., 1.)),
                Sprite {
                    color: materials.green,
                    custom_size: Some(Vec2::new(32.0, 8.0)),
                    .. default()
                }
            ))
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
            black: Color::srgba(0., 0., 0., 0.33),
            green: Color::srgba(0., 1., 0., 0.33),
            yellow: Color::srgba(1., 1., 0., 0.33),
            red: Color::srgb(1., 0., 0.),
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
    let window = window_query.single().expect("No primary window.");
    let height = window.height();
    let width = window.width();

    commands.spawn(Camera2d::default()).insert(MainCamera);
    commands.spawn((
        Transform::from_xyz(-width/2., height/2., 0.),
        TextFont {
            font: materials.font.clone(),
            font_size: 35.,
            ..Default::default()
        },
        TextColor(Color::srgb(0.5, 0.5, 1.0)),
        Text2d::new("Attack: ¤"),
        Anchor::TopLeft,
    )).with_children(|p| {
        p.spawn((AttackMoneyText, TextSpan::new(""), TextFont {
            font: materials.font.clone(),
            font_size: 35.,
            ..Default::default()
        },
        TextColor(Color::srgb(0.5, 0.5, 1.0)),));
        p.spawn((TextSpan::new("\nDefence: ¤"), TextFont {
            font: materials.font.clone(),
            font_size: 35.,
            ..Default::default()
        },
        TextColor(Color::srgb(0.5, 0.5, 1.0))));
        p.spawn((DefenceMoneyText, TextSpan::new(""), TextFont {
            font: materials.font.clone(),
            font_size: 35.,
            ..Default::default()
        },
        TextColor(Color::srgb(0.5, 0.5, 1.0)),));
    });
    let zone_x = width/2.-SPAWN_WIDTH/2.;
    commands.spawn((
        Transform::from_xyz(-zone_x, 0., 0.),
        Sprite {
            color: materials.yellow.with_alpha(1.),
            custom_size: Some(Vec2::new(SPAWN_WIDTH, height)),
            .. default()
        }
    ));
    commands.spawn((
        Transform::from_xyz(zone_x, 0., 0.),
        Sprite {
            color: materials.yellow.with_alpha(1.),
            custom_size: Some(Vec2::new(SPAWN_WIDTH, height)),
            .. default()
        }
    ));
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
    let (camera, camera_transform) = camera_q.single().unwrap();

    let window = window_query.single().unwrap();

    mouse_loc.0 = window.cursor_position().unwrap_or(Vec2::ZERO);
    mouse_loc.0 = camera.viewport_to_world_2d(camera_transform, mouse_loc.0).unwrap();
}


fn fighter_movement(
    time: Res<Time>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Transform, &Sprite, &Fighter)>,
) {
    let window = window_query.single().expect("No primary window.");
    let height = window.height();

    let delta = time.delta_secs();

    query.par_iter_mut().for_each(|(mut transform, sprite, fighter)| {
        if !fighter.moving() {
            return
        }

        let scale_x = if sprite.flip_x { -1. } else { 1. };
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
    let window = window_query.single().expect("No primary window.");
    let width = window.width();

    let (camera, global_transform) = camera_q.single().unwrap();

    for (ent, transform, fighter) in query.iter() {
        let pos = camera.world_to_viewport(global_transform, transform.translation).unwrap();
        if pos.x > width {
            commands.entity(ent).despawn();
            money.left += fighter.skills.siege as i16;
        } else if pos.x < 0. {
            commands.entity(ent).despawn();
            money.right += fighter.skills.siege as i16;
        }
    }
}

#[derive(Component)]
struct AttackMoneyText;
#[derive(Component)]
struct DefenceMoneyText;

fn scoreboard_text_system(
    atk: Query<&mut TextSpan, (With<AttackMoneyText>, Without<DefenceMoneyText>)>,
    def: Query<&mut TextSpan, (With<DefenceMoneyText>, Without<AttackMoneyText>)>,
    money: Res<Money>
) {
    for mut text in atk {
        text.0 = format!("{}", money.left);
    }
    for mut text in def {
        text.0 = format!("{}", money.right);
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
        let collision = Aabb2d::new(
            left_trans.translation.xy(),
            left_spr.custom_size.unwrap() / 2.).intersects(&Aabb2d::new(
            right_trans.translation.xy(),
            right_spr.custom_size.unwrap() / 2.)
        );
        if collision {
            if left_spr.flip_x == right_spr.flip_x {
                let (wait_fighter, wait_entity) = if left_spr.flip_x ^ (left_trans.translation.x < right_trans.translation.x) {
                    (left_fighter, left_entity)
                } else {
                    (right_fighter, right_entity)
                };
                wait_fighter.waiting = true;
                waiting.insert(wait_entity.clone(), true);
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

    let delta = time.delta_secs();

    query
        .par_iter_mut().for_each(move |(ent, mut fighter, _)| {
            fighter.attack_cooldown -= delta;
            if fighter.attack_cooldown <= 0. {
                fighter.attack_cooldown = 0.;
                if let Some(fighting) = fighter.fighting {
                    tx.send((ent, fighting, fighter.skills)).unwrap();
                }
            }
        });

    let mut rng = rand::rng();

    for (fighter, fought_ent, skills) in rx.into_iter() {
        if let Ok((_, mut fought, f_trans)) = query.get_mut(fought_ent) {
            if rng.random_range(0..=skills.attack) > rng.random_range(0..=fought.skills.defence) {
                let dmg = rng.random_range(1..=skills.strength);

                let actual_dmg = dmg.saturating_sub(rng.random_range(0..=fought.protection));

                fought.hp = fought.hp.saturating_sub(actual_dmg);

                let mut transform = Transform::from_translation(f_trans.translation);

                transform.translation.y += 45.;
                transform.translation.z += 1.;

                let ent = commands.spawn((
                    Text2d(format!("{}", actual_dmg)),
                    TextFont {
                        font: materials.font.clone(),
                        font_size: 18.,
                        ..Default::default()
                    },
                    TextColor(Color::BLACK),
                    transform.clone() * Transform::from_translation(Vec3::new(0., 0., 2.)),
                )).id();
                commands.spawn((
                    transform,
                    Sprite {
                        color: materials.red,
                        custom_size: Some(Vec2::new(15., 15.)),
                        .. default()
                    },
                )).insert(Timeout::new(1.15).tied_to(vec![ent]));

                if fought.hp <= 0 {
                    if f_trans.scale.x > 0. {
                        money.left += 1;
                    } else {
                        money.right += 1;
                    }
                    commands.entity(fought_ent).despawn();
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
    mouse_button: Res<ButtonInput<MouseButton>>,
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

    spawn_zone.timer -= time.delta_secs();

    while spawn_zone.timer < 0. && money.right >= 1 {
        let denominator = (money.right / 7).max(1) as f32;
        spawn_zone.timer += 1. / denominator;
        let mut rng = rand::rng();
        let y = rng.random_range(-spawn_zone.height/2. .. spawn_zone.height/2.);
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
        let time = time.delta_secs();
        timeout.time_left -= time;
        if timeout.time_left <= 0. {
            commands.entity(ent).despawn();
            for &ent in &timeout.tied_to {
                commands.entity(ent).despawn();
            }
        }
    }
}
