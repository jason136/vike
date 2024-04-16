mod camera;
mod debug;
mod game_object;
mod hdr;
mod renderer;
mod resources;
mod texture;

use std::f32::consts::PI;

use anyhow::Result;
use game_object::{GameObjectStore, Transform3D};
use glam::{Quat, Vec3};
use instant::{Duration, Instant};
use renderer::Renderer;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::CursorGrabMode;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::camera::CameraController;

const MAX_LIGHTS: usize = 128;
const MAX_INSTANCES: usize = 131072;

async fn create_game_objects(renderer: &Renderer) -> Result<GameObjectStore> {
    let mut game_objects = GameObjectStore::new();

    let cube_model = game_objects.load_model("cube.obj", renderer).await?;
    // let basemesh_model = Arc::new(load_model("basemesh.obj", renderer).await.unwrap());
    // let smooth_vase_model = Arc::new(load_model("smooth_vase.obj", renderer).await.unwrap());
    // let flat_vase_model = Arc::new(load_model("flat_vase.obj", renderer).await.unwrap());
    // let floor_model = Arc::new(load_model("quad.obj", renderer).await.unwrap());

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
        Box::new(|i: u32| {
            Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }
        }),
    );

    game_objects.new_light(
        "green",
        Transform3D {
            position: Vec3::new((2.0 * PI / 3.0).sin() * 64.0, 2.0, (2.0 * PI / 3.0).cos() * 64.0),
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
        Box::new(|i: u32| {
            Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }
        }),
    );

    game_objects.new_light(
        "blue",
        Transform3D {
            position: Vec3::new((4.0 * PI / 3.0).sin() * 64.0, 2.0, (4.0 * PI / 3.0).cos() * 64.0),
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
        Box::new(|i: u32| {
            Transform3D {
                position: Vec3::new(0.0, 20.0 * i as f32, 0.0),
                ..Default::default()
            }
        }),
    );

    // models.insert("basemesh", basemesh_model);
    // models.insert("smooth_vase", smooth_vase_model);
    // models.insert("flat_vase", flat_vase_model);
    // models.insert("floor", floor_model);

    Ok(game_objects)
}

fn animate_game_objects(game_objects: &mut GameObjectStore, dt: Duration) {
    let dt_secs = dt.as_secs_f32();

    for (_, light) in game_objects.lights_mut() {
        light.transform.position =
            Quat::from_axis_angle(Vec3::Y, dt_secs * 0.5) * light.transform.position;
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Vike")
        .with_inner_size(LogicalSize::new(800.0, 600.0))
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        use winit::dpi::PhysicalSize;
        window
            .request_inner_size(PhysicalSize::new(450, 400))
            .unwrap();

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas().unwrap());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    let mut renderer = Renderer::new(window).await;

    let mut game_objects = create_game_objects(&renderer).await.unwrap();

    let mut camera_controller = CameraController::new(4.0, 0.6);
    let mut focused = true;

    let mut last_instant = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::AboutToWait => {
                renderer.window().request_redraw();
            }
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == renderer.window().id() && !renderer.input(event) => match event {
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let dt = now - last_instant;
                    last_instant = now;

                    animate_game_objects(&mut game_objects, dt);
                    camera_controller.update_camera(&mut renderer.camera, dt);

                    match renderer.render(&game_objects) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            renderer.resize(renderer.size())
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        match code {
                            KeyCode::Escape => {
                                focused = false;
                                renderer.window().set_cursor_visible(true);
                                renderer
                                    .window()
                                    .set_cursor_grab(CursorGrabMode::None)
                                    .unwrap();
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
                    focused = true;
                    renderer.window().set_cursor_visible(false);
                    renderer
                        .window()
                        .set_cursor_grab(CursorGrabMode::Locked)
                        .unwrap();
                }
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    renderer.resize(renderer.window().inner_size());
                }
                WindowEvent::Focused(focus) => {
                    focused = *focus;
                    renderer.window().set_cursor_visible(!focused);
                    if focused {
                        renderer
                            .window()
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .unwrap();
                    } else {
                        renderer
                            .window()
                            .set_cursor_grab(CursorGrabMode::None)
                            .unwrap();
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                _ => (),
            },
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if focused {
                    camera_controller.process_mouse(delta.0, delta.1);
                }
            }
            _ => (),
        })
        .unwrap();
}
