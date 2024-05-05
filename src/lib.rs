#![feature(unboxed_closures)]

pub mod camera;
pub mod debug;
pub mod game_object;
pub mod hdr;
pub mod renderer;
pub mod resources;
pub mod texture;

use std::borrow::BorrowMut;
use std::future::Future;
use std::pin::Pin;

use game_object::GameObjectStore;
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

pub async fn run(
    title: &str,
    mut setup_fn: impl for<'a> FnMut(
        &'a mut GameObjectStore,
        &'a mut CameraController,
        &'a Renderer,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
    update_fn: impl Fn(&mut GameObjectStore, &mut CameraController, Duration),
) {
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
        .with_title(title)
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
                let dst = doc.get_element_by_id(title)?;
                let canvas = web_sys::Element::from(window.canvas().unwrap());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    let mut renderer = Renderer::new(window).await;

    let mut game_objects = GameObjectStore::new();
    let mut camera_controller = CameraController::new(4.0, 0.6);
    (setup_fn)(&mut game_objects, &mut camera_controller, &renderer).await;
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
            } if window_id == renderer.window().id() && !renderer.borrow_mut().input(event) => {
                match event {
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = now - last_instant;
                        last_instant = now;

                        (update_fn)(&mut game_objects, &mut camera_controller, dt);
                        camera_controller.update_camera(&mut renderer.camera, dt);

                        match renderer.render(&mut game_objects) {
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
                }
            }
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
