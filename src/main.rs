use std::collections::HashMap;
use self_compare::SliceCompareExt;

use rand::Rng;

use bevy::{
    prelude::*,
    tasks::{ComputeTaskPool, ParallelIterator},
    input::system::exit_on_esc_system,
    render::pass::ClearColor,
    sprite::collide_aabb::{collide, Collision},
};

fn main() {
    App::build()
        .add_resource(ClearColor(Color::rgb(0.24, 0.5, 0.01)))
        .add_resource(WindowDescriptor {
            title: "Sidewars".to_owned(),
            .. Default::default()
        })
        .add_resource(MouseLoc(Default::default()))
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
    fighting: Option<Entity>,
    attack_cooldown: f32,
    waiting: bool,
}

impl Fighter {
    pub fn new(skills: Skills) -> Self {
        Fighter {
            hp: skills.hp,
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

struct HealthBar;

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
    cmds.spawn(fighter_sprite_bundle(x, y, flipped, materials))
        .with(Fighter::new(skills))
        .with_children(|parent| {
            parent
                .spawn(SpriteBundle {
                    material: materials.black.clone(),
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite::new(Vec2::new(34.0, 10.0)),
                    ..Default::default()
                })
                .spawn(SpriteBundle {
                    material: materials.green.clone(),
                    transform: Transform::from_translation(Vec3::new(0., 30., 1.)),
                    sprite: Sprite::new(Vec2::new(32.0, 8.0)),
                    ..Default::default()
                })
                .with(HealthBar)
                ;
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

impl FromResources for Materials {
    fn from_resources(resources: &Resources) -> Self {
        let asset_server = resources.get::<AssetServer>().unwrap();
        let mut materials = resources.get_mut::<Assets<ColorMaterial>>().unwrap();
        Self {
            font: asset_server.load("DroidSansMono.ttf"),
            fighter: materials.add(asset_server.load("fighter.png").into()),
            black: materials.add(Color::rgba(0., 0., 0., 0.33).into()),
            green: materials.add(Color::rgba(0., 1., 0., 0.33).into()),
            red: materials.add(Color::rgb(1., 0., 0.).into()),
        }
    }
}

fn setup(
    commands: &mut Commands,
    materials: Res<Materials>,
) {
    commands
        .spawn(Camera2dBundle::default())
        .spawn(CameraUiBundle::default())
        // TODO: Make player a triangle instead
        .spawn(TextBundle {
            text: Text {
                font: materials.font.clone(),
                value: "Score:".to_string(),
                style: TextStyle {
                    color: Color::rgb(0.5, 0.5, 1.0),
                    font_size: 40.0,
                    ..Default::default()
                },
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
        })
        .with(Scoreboard { score: 0 })
        ;
}

#[derive(Debug, Default, Copy, Clone)]
pub struct MouseLoc(Vec2);

#[derive(Default)]
pub struct MouseLocationState {
    mouse_motion_event_reader: EventReader<CursorMoved>,
}

fn mouse_location_system(
    mut state: Local<MouseLocationState>,
    windows: Res<Windows>,
    mouse_motion_events: Res<Events<CursorMoved>>,
    mut mouse_loc: ResMut<MouseLoc>,
) {
    let window = windows.get_primary().unwrap();
    let primary_id = window.id();
    let w = window.width() / 2.;
    let h = window.height() / 2.;
    for cm in state.mouse_motion_event_reader.iter(&mouse_motion_events).filter(|cm| cm.id == primary_id).last() {
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

    query.par_iter_mut(32).filter(|(_, fighter)| fighter.moving()).for_each(&pool, |(mut transform, fighter)| {
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
        text.value = format!("Score: {}", scoreboard.score);
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
    commands: &mut Commands,
    pool: Res<ComputeTaskPool>,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Fighter)>
) {
    let (tx, rx) = sync_channel(query.iter_mut().len());

    let delta = time.delta_seconds();

    query
        .par_iter_mut(32)
        .for_each(&pool, move |(ent, mut fighter)| {
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
        if let Ok((_, mut fought)) = query.get_mut(fought_ent) {
            if rng.gen_range(0..=skills.attack) > rng.gen_range(0..=fought.skills.defence) {
                let dmg = rng.gen_range(1..=skills.strength);
                fought.hp = fought.hp.saturating_sub(dmg);

                if fought.hp <= 0 {
                    commands.despawn_recursive(fought_ent);
                }
            }
        } else {
            let (_, mut fighter) = query.get_mut(fighter).unwrap();
            fighter.fighting = None;
        }
        let (_, mut fighter) = query.get_mut(fighter).unwrap();
        fighter.attack_cooldown += COOLDOWN;
    }
}

fn soldier_placement_system(
    commands: &mut Commands,
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

        spawn_fighter(commands, mouse_loc.0.x, mouse_loc.0.y, flipped, &materials, Skills {
            attack: 30,
            defence: 10,
            hp: 20,
            strength: 5,
            speed: 5,
        });
    }
}