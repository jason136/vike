#![feature(unboxed_closures)]
#![feature(let_chains)]
#![feature(type_alias_impl_trait)]

use std::borrow::BorrowMut;
use std::sync::Arc;

use game_object::GameObjectStore;
use image::{ImageBuffer, Rgba};
use renderer::{RenderTarget, Renderer};
use web_time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::DeviceEvent;
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::camera::CameraController;

pub mod camera;
pub mod debug;
pub mod game_object;
pub mod hdr;
pub mod renderer;
pub mod resources;
pub mod texture;

const MAX_LIGHTS: usize = 128;
const MAX_INSTANCES: usize = 131072;

pub enum RenderMode {
    Window,
    Headless,
}

pub async fn run_windowed(
    title: &str,
    width: u32,
    height: u32,
    setup_fn: impl Fn(&mut GameObjectStore, &mut CameraController, &Renderer),
    update_fn: impl Fn(&mut GameObjectStore, &mut CameraController, Duration),
    window_event_fn: impl Fn(
        &Arc<Window>,
        &mut GameObjectStore,
        &mut CameraController,
        &mut Renderer,
        &WindowEvent,
        &EventLoopWindowTarget<()>,
    ),
    device_event_fn: impl Fn(&mut GameObjectStore, &mut CameraController, DeviceEvent),
) {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width, height))
            .with_resizable(true)
            .build(&event_loop)
            .unwrap(),
    );

    #[cfg(target_arch = "wasm32")]
    {
        use winit::dpi::PhysicalSize;
        window
            .request_inner_size(PhysicalSize::new(width, height))
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

    let mut renderer = Renderer::new(RenderTarget::Window(window.clone())).await;

    let mut game_objects = GameObjectStore::default();
    let mut camera_controller = CameraController::new(4.0, 0.6);

    (setup_fn)(&mut game_objects, &mut camera_controller, &renderer);

    let mut last_instant = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::AboutToWait => {
                window.request_redraw();
            }
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == window.id() && !renderer.borrow_mut().input(event) => match event {
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let dt = now - last_instant;
                    last_instant = now;

                    (update_fn)(&mut game_objects, &mut camera_controller, dt);
                    camera_controller.update_camera(&mut renderer.camera, dt);

                    match renderer.render(&game_objects) {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            renderer.resize(window.inner_size());
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                    }
                }
                _ => window_event_fn(
                    &window,
                    &mut game_objects,
                    &mut camera_controller,
                    &mut renderer,
                    event,
                    elwt,
                ),
            },
            Event::DeviceEvent { event, .. } => {
                device_event_fn(&mut game_objects, &mut camera_controller, event);
            }
            _ => (),
        })
        .unwrap();
}

pub async fn run_headless(
    width: u32,
    height: u32,
    setup_fn: impl Fn(&mut GameObjectStore, &mut CameraController, &Renderer),
    update_fn: impl Fn(&mut GameObjectStore, &mut CameraController, Duration),
    frame_fn: impl Fn(ImageBuffer<Rgba<u8>, Vec<u8>>),
) {
    let mut renderer = Renderer::new(RenderTarget::Headless { width, height }).await;

    let mut game_objects = GameObjectStore::default();
    let mut camera_controller = CameraController::new(4.0, 0.6);

    (setup_fn)(&mut game_objects, &mut camera_controller, &renderer);

    let mut last_instant = Instant::now();

    while renderer.render(&game_objects).is_ok() {
        let now = Instant::now();
        let dt = now - last_instant;
        last_instant = now;

        (update_fn)(&mut game_objects, &mut camera_controller, dt);

        camera_controller.update_camera(&mut renderer.camera, dt);

        renderer.render(&game_objects).unwrap();

        (frame_fn)(renderer.image_buffer().await.unwrap());
    }
}
