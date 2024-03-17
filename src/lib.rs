mod camera;
mod game_object;
mod renderer;
mod resources;
mod texture;

use game_object::GameObjectType;
use instant::Duration;
use nalgebra::{Rotation3, Vector3};
use renderer::Renderer;
use std::{collections::HashMap, sync::Arc};
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::camera::CameraController;
use crate::game_object::{GameObject, Model};
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

fn animate_game_objects(game_objects: &mut HashMap<u32, GameObject>, dt: Duration) {
    let dt_secs = dt.as_secs_f32();
    for obj in game_objects.values_mut() {
        match obj.obj {
            GameObjectType::PointLight { .. } => {
                let rotation = Rotation3::from_axis_angle(&Vector3::y_axis(), dt_secs);
                obj.transform.translation = rotation * obj.transform.translation;
            }
            GameObjectType::Model { .. } => {
                let rotation = Rotation3::from_axis_angle(&Vector3::y_axis(), dt_secs * 0.01);

                obj.transform.translation = rotation * obj.transform.translation;
                obj.transform.rotation.y += dt_secs * 0.1;
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

    let mut camera_controller = CameraController::new(4.0, 0.6);
    let mut focused = true;

    let mut last_render_time = instant::Instant::now();

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
                    let now = instant::Instant::now();
                    let dt = now - last_render_time;
                    last_render_time = now;

                    animate_game_objects(&mut game_objects, dt);
                    camera_controller.update_camera(&mut renderer.camera, dt);
                    renderer.update(dt);

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
                            KeyCode::Escape => {
                                focused = !focused;
                                renderer.window.set_cursor_visible(!focused);
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
                    state,
                    ..
                } => focused = state == &ElementState::Pressed,
                WindowEvent::Resized(physical_size) => {
                    renderer.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    renderer.resize(renderer.window.inner_size());
                }
                WindowEvent::Focused(focus) => {
                    focused = *focus;
                    renderer.window.set_cursor_visible(!focused);
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
                    camera_controller.process_mouse(delta.0, delta.1)
                }
            }
            _ => (),
        })
        .unwrap();
}
