#![feature(async_closure)]

use std::f32::consts::PI;

use glam::{Quat, Vec3};
use vike::{
    camera::CameraController, game_object::{self, GameObjectStore, Transform3D}, renderer::Renderer, run
};

fn main() {
    async fn setup(game_objects: &mut GameObjectStore, camera_controller: &mut CameraController, renderer: &Renderer) {
        let cube_model = game_objects.load_model("cube.obj", renderer).await.unwrap();

        game_objects.new_game_object(
            "cube",
            Transform3D {
                position: Vec3::new(0.0, 0.0, 0.0),
                scale: Vec3::new(1.0, 1.0, 1.0),
                ..Default::default()
            },
            Some(cube_model.clone()),
        );
        game_objects.new_array(
            "cube",
            "cube array",
            100,
            Box::new(|i: u32| {
                let z = i / 10;
                let x = i % 10;
                let x = 3.0 * (x as f32 - 10_f32 / 2.0);
                let z = 3.0 * (z as f32 - 10_f32 / 2.0);
                let position = Vec3::new(x, 0.0, z);

                let rotation = if position == Vec3::ZERO {
                    Quat::from_axis_angle(Vec3::Z, 0.0)
                } else {
                    Quat::from_axis_angle(position.normalize(), 45.0)
                };

                Transform3D {
                    position,
                    rotation: rotation.to_euler(glam::EulerRot::XYZ).into(),
                    ..Default::default()
                }
            }),
        );
        game_objects.new_array(
            "cube",
            "cube spiral",
            10000,
            Box::new(|i: u32| {
                let position =
                    Quat::from_axis_angle(Vec3::Y, i as f32 * 0.25) * Vec3::new(10.0, 0.0, 10.0);

                Transform3D {
                    position: position + Vec3::new(0.0, i as f32 * 0.25, 0.0),
                    ..Default::default()
                }
            }),
        );

        game_objects.new_light(
            "red",
            Transform3D {
                position: Vec3::new(0.0, 2.0, 64.0),
                scale: Vec3::new(0.25, 0.25, 0.25),
                ..Default::default()
            },
            Some(cube_model.clone()),
            Vec3::new(1.0, 0.0, 0.0),
            1000.0,
        );
        game_objects.new_array(
            "red",
            "red",
            42,
            Box::new(|i: u32| Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }),
        );

        game_objects.new_light(
            "green",
            Transform3D {
                position: Vec3::new(
                    (2.0 * PI / 3.0).sin() * 64.0,
                    2.0,
                    (2.0 * PI / 3.0).cos() * 64.0,
                ),
                scale: Vec3::new(0.25, 0.25, 0.25),
                ..Default::default()
            },
            Some(cube_model.clone()),
            Vec3::new(0.0, 1.0, 0.0),
            1000.0,
        );
        game_objects.new_array(
            "green",
            "green",
            42,
            Box::new(|i: u32| Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }),
        );

        game_objects.new_light(
            "blue",
            Transform3D {
                position: Vec3::new(
                    (4.0 * PI / 3.0).sin() * 64.0,
                    2.0,
                    (4.0 * PI / 3.0).cos() * 64.0,
                ),
                scale: Vec3::new(0.25, 0.25, 0.25),
                ..Default::default()
            },
            Some(cube_model.clone()),
            Vec3::new(0.0, 0.0, 1.0),
            1000.0,
        );
        game_objects.new_array(
            "blue",
            "blue",
            42,
            Box::new(|i: u32| Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }),
        );
    }

    pollster::block_on(run(
        "Vike",
        |game_objects, camera_controller, renderer| Box::pin(setup(game_objects, camera_controller, renderer)),
        |game_objects, _camera_controller, dt| {
            let dt_secs = dt.as_secs_f32();

            for (_, light) in game_objects.lights_mut() {
                light.transform.position =
                    Quat::from_axis_angle(Vec3::Y, dt_secs * 0.5) * light.transform.position;
            }
        },
    ));
}
