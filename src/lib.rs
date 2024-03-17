mod camera;
mod game_object;
mod movement;
mod resources;
mod texture;
// mod render_systems;
mod renderer;

use game_object::GameObjectType;
use nalgebra::{Rotation3, Vector3};
use renderer::Renderer;
use std::{collections::HashMap, sync::Arc, time::Instant};
use winit::dpi::LogicalSize;
use winit::event::MouseButton;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    dpi::LogicalPosition,
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::game_object::{GameObject, Model};
use crate::movement::KeyboardController;
use crate::resources::load_model;

async fn create_game_objects(
    renderer: &Renderer,
) -> (HashMap<u32, GameObject>, HashMap<&'static str, Arc<Model>>) {
    let mut game_objects: HashMap<u32, GameObject> = HashMap::new();
    let mut models: HashMap<&str, Arc<Model>> = HashMap::new();

    let cube_model = Arc::new(load_model("cube.obj", renderer).await.unwrap());
    // let basemesh_model = Arc::new(load_model("basemesh.obj", renderer).await.unwrap());
    // let smooth_vase_model = Arc::new(load_model("smooth_vase.obj", renderer).await.unwrap());
    // let flat_vase_model = Arc::new(load_model("flat_vase.obj", renderer).await.unwrap());
    // let floor_model = Arc::new(load_model("quad.obj", renderer).await.unwrap());

    let mut game_object = GameObject::new(Some(cube_model.clone()));
    game_object.transform.translation = [-0.5, 0.5, 0.0].into();
    game_object.transform.scale = [2.0; 3].into();
    game_objects.insert(game_object.id, game_object);

    // let mut game_object = GameObject::new(Some(cube_model.clone()));
    // game_object.transform.translation = [0.5, 0.5, 0.0].into();
    // game_object.transform.scale = [2.0; 3].into();
    // game_objects.insert(game_object.id, game_object);

    // let mut game_object = GameObject::new(Some(floor_model.clone()));
    // game_object.transform.translation = [0.0, 0.5, 0.0].into();
    // game_object.transform.scale = [3.0, 1.0, 3.0].into();
    // game_objects.insert(game_object.id, game_object);

    // let light_colors = [
    //     Vector3::new(1.0, 0.1, 0.1),
    //     Vector3::new(0.1, 0.1, 1.0),
    //     Vector3::new(0.1, 1.0, 0.1),
    //     Vector3::new(1.0, 1.0, 0.1),
    //     Vector3::new(0.1, 1.0, 1.0),
    //     Vector3::new(1.0, 1.0, 1.0),
    // ];

    let point_light = GameObject::new_point_light(Some(cube_model.clone()));
    game_objects.insert(point_light.id, point_light);

    // for i in 0..light_colors.len() {
    //     let mut point_light = GameObject::new_point_light(0.5, 0.1, light_colors[i]);

    //     let rotation = Rotation3::from_axis_angle(
    //         &Vector3::y_axis(),
    //         i as f32 * std::f32::consts::PI * 2.0 / light_colors.len() as f32,
    //     );

    //     point_light.transform.translation = rotation * Vector3::new(-1.0, -1.0, -1.0);

    //     game_objects.insert(point_light.id, point_light);
    // }

    // for i in 0..1000 {
    //     let mut game_object = GameObject::new(Some(basemesh_model.clone()));
    //     game_object.transform.translation = [0.0, 0.0, 0.0].into();
    //     // game_object.transform.rotation = [std::f32::consts::PI, 0.0, 0.0].into();
    //     // game_object.transform.scale = [0.1; 3].into();

    //     game_object.transform.rotation.y += i as f32 * 0.01;
    //     game_objects.insert(game_object.id, game_object);
    // }

    models.insert("cube", cube_model);
    // models.insert("basemesh", basemesh_model);
    // models.insert("smooth_vase", smooth_vase_model);
    // models.insert("flat_vase", flat_vase_model);
    // models.insert("floor", floor_model);

    (game_objects, models)
}

fn animate_game_objects(game_objects: &mut HashMap<u32, GameObject>, dt: f32) {
    for obj in game_objects.values_mut() {
        match obj.obj {
            GameObjectType::PointLight { .. } => {
                let rotation = Rotation3::from_axis_angle(&Vector3::y_axis(), dt);
                obj.transform.translation = rotation * obj.transform.translation;
            }
            GameObjectType::Model { .. } => {
                let rotation = Rotation3::from_axis_angle(&Vector3::y_axis(), dt * 0.01);

                obj.transform.translation = rotation * obj.transform.translation;
                obj.transform.rotation.y += dt * 0.1;
            }
        }
    }

    // let mut game_object = GameObject::new(Some(models["basemesh"].clone()));
    // game_object.transform.translation = [4.0, -1.0, 0.0].into();
    // game_object.transform.rotation = [std::f32::consts::PI, 0.0, 0.0].into();
    // game_object.transform.scale = [0.1; 3].into();
    // game_objects.insert(game_object.id, game_object);
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

    let (mut game_objects, _models) = create_game_objects(&renderer).await;

    let mut camera_controller = KeyboardController::new();

    let mut current_time = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::AboutToWait => {
                renderer.window.request_redraw();
            }
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == renderer.window.id() && !renderer.input(event) => match event {
                WindowEvent::RedrawRequested => {
                    let new_time = Instant::now();
                    let delta_time = new_time.duration_since(current_time).as_secs_f32();
                    current_time = new_time;

                    println!("FPS: {}", 1.0 / delta_time);

                    let dimensions = renderer.size;
                    camera_controller
                        .move_xz(delta_time, renderer.camera.transform.as_mut().unwrap());
                    if camera_controller.mouse_engaged {
                        renderer.window.set_cursor_visible(false);
                        renderer
                            .window
                            .set_cursor_position(LogicalPosition::new(
                                dimensions.width / 4,
                                dimensions.height / 4,
                            ))
                            .unwrap();
                    } else {
                        renderer.window.set_cursor_visible(true);
                    }

                    renderer.camera.match_transform();

                    let aspect = dimensions.width as f32 / dimensions.height as f32;
                    renderer.camera.set_perspective_projection(
                        50.0_f32.to_radians(),
                        aspect,
                        0.1,
                        500.0,
                    );

                    animate_game_objects(&mut game_objects, delta_time);

                    renderer.update();
                    match renderer.render(&game_objects) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => renderer.resize(renderer.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        match code {
                            KeyCode::KeyA => {
                                camera_controller.move_left = event.state == ElementState::Pressed
                            }
                            KeyCode::KeyD => {
                                camera_controller.move_right = event.state == ElementState::Pressed
                            }
                            KeyCode::KeyW => {
                                camera_controller.move_forward =
                                    event.state == ElementState::Pressed
                            }
                            KeyCode::KeyS => {
                                camera_controller.move_backward =
                                    event.state == ElementState::Pressed
                            }
                            KeyCode::Space => {
                                camera_controller.move_up = event.state == ElementState::Pressed
                            }
                            KeyCode::ShiftLeft => {
                                camera_controller.move_down = event.state == ElementState::Pressed
                            }
                            KeyCode::ArrowLeft => {
                                camera_controller.look_left = event.state == ElementState::Pressed
                            }
                            KeyCode::ArrowRight => {
                                camera_controller.look_right = event.state == ElementState::Pressed
                            }
                            KeyCode::ArrowUp => {
                                camera_controller.look_up = event.state == ElementState::Pressed
                            }
                            KeyCode::ArrowDown => {
                                camera_controller.look_down = event.state == ElementState::Pressed
                            }
                            KeyCode::Escape => {
                                camera_controller.disable_mouse_engaged =
                                    event.state == ElementState::Pressed
                            }
                            _ => {}
                        };
                    };
                }
                WindowEvent::CursorMoved { position, .. } => {
                    camera_controller.mouse_delta = (position.x, position.y);
                }
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state,
                    ..
                } => camera_controller.focused = state == &ElementState::Pressed,
                WindowEvent::Focused(focused) => {
                    camera_controller.focused = *focused;
                }
                WindowEvent::CursorEntered { .. } => {
                    camera_controller.cursor_in_window = true;
                }
                WindowEvent::CursorLeft { .. } => {
                    camera_controller.cursor_in_window = false;
                }
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    renderer.resize(renderer.window.inner_size());
                }
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                _ => (),
            },
            _ => (),
        })
        .unwrap();
}