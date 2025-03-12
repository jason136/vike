use std::{f32::consts::PI, sync::Arc, time::Duration};

use image::{ImageBuffer, Rgba};
use vike::{
    camera::CameraController,
    game_object::{GameObjectStore, Quat, Transform3D, Vec3},
    renderer::Renderer,
    run_headless, run_windowed,
};
use winit::{
    event::{DeviceEvent, MouseButton, WindowEvent},
    event_loop::EventLoopWindowTarget,
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

fn setup(
    game_objects: &mut GameObjectStore,
    camera_controller: &mut CameraController,
    renderer: &Renderer,
) {
    camera_controller.sensitivity = 0.4;
    camera_controller.speed = 6.0;

    let cube_model = game_objects.load_model("cube.obj", renderer).unwrap();

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

fn update(
    game_objects: &mut GameObjectStore,
    camera_controller: &mut CameraController,
    dt: Duration,
) {
    let dt_secs = dt.as_secs_f32();

    println!("dt: {:?}", dt);

    for (_, light) in game_objects.lights_mut() {
        light.transform.position =
            Quat::from_axis_angle(Vec3::Y, dt_secs * 0.5) * light.transform.position;
    }
}

fn window_event(
    window: &Arc<Window>,
    game_objects: &mut GameObjectStore,
    camera_controller: &mut CameraController,
    renderer: &mut Renderer,
    event: &WindowEvent,
    elwt: &EventLoopWindowTarget<()>,
) {
    println!("{:?}", event);

    match event {
        WindowEvent::KeyboardInput { event, .. } => {
            if let PhysicalKey::Code(code) = event.physical_key {
                match code {
                    KeyCode::Escape => {
                        camera_controller.focused = false;
                        window.set_cursor_visible(true);
                        window.set_cursor_grab(CursorGrabMode::None).unwrap_or(());
                    }
                    _ => camera_controller.process_keyboard(code, event.state),
                }
                camera_controller.process_keyboard(code, event.state);
            };
        }
        WindowEvent::MouseWheel { delta, .. } => {
            camera_controller.process_scroll(delta);
        }
        WindowEvent::MouseInput {
            button: MouseButton::Left,
            ..
        } => {
            camera_controller.focused = true;
            window.set_cursor_visible(false);
            window.set_cursor_grab(CursorGrabMode::Locked).unwrap_or(());
        }
        WindowEvent::Resized(physical_size) => {
            renderer.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { .. } => {
            renderer.resize(window.inner_size());
        }
        WindowEvent::Focused(focus) => {
            camera_controller.focused = *focus;
            window.set_cursor_visible(!camera_controller.focused);
            if camera_controller.focused {
                window.set_cursor_grab(CursorGrabMode::Locked).unwrap_or(());
            } else {
                window.set_cursor_grab(CursorGrabMode::None).unwrap_or(());
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        WindowEvent::CloseRequested => {
            elwt.exit();
        }
        _ => (),
    }
}

fn device_event(
    _game_objects: &mut GameObjectStore,
    camera_controller: &mut CameraController,
    event: DeviceEvent,
) {
    println!("{:?}", event);

    match event {
        DeviceEvent::MouseMotion { delta, .. } if camera_controller.focused => {
            camera_controller.process_mouse(delta.0, delta.1);
        }
        _ => (),
    }
}

fn handle_frame(image_buffer: ImageBuffer<Rgba<u8>, Vec<u8>>) {
    println!("saving image");
    image_buffer.save("image.png").unwrap();
    unimplemented!()
}

fn main() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }

    // pollster::block_on(run_windowed(
    //     "Vike",
    //     800,
    //     600,
    //     setup,
    //     update,
    //     window_event,
    //     device_event,
    // ));

    pollster::block_on(run_headless(800, 600, setup, update, handle_frame));
}
