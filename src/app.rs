use crate::{
    simple_render_system::{SimpleRenderSystem},
    renderer::Renderer,
    game_object::{GameObject, Model},
    camera::Camera,
    movement::KeyboardController,
};

use std::{sync::{Arc, Mutex}, time::Instant};
use winit::{
    event::{Event, WindowEvent, ElementState, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
};
use std::io::Write;

fn create_game_objects(renderer: &Renderer) -> Vec<GameObject> {
    let cube_model = Arc::new(Model::load_obj(renderer, "models/flat_vase.obj"));

    let mut game_objects = vec![];

    let mut game_object = GameObject::new(Some(cube_model));
    game_object.transform.translation = [0.0, 0.5, 2.5].into();
    game_object.transform.scale = [3.0; 3].into();
    game_objects.push(game_object);
    
    game_objects
}

fn animate_game_objects(game_objects: Arc<Mutex<Vec<GameObject>>>, dt: f32) {
    // for obj in game_objects.lock().unwrap().iter_mut() {
    //     obj.transform.rotation.y += 1.0 * dt * std::f32::consts::PI * 2.0;
    //     obj.transform.rotation.x += 0.5 * dt * std::f32::consts::PI * 2.0;
    // }
}

pub struct VkApp {
    pub event_loop: EventLoop<()>,
    pub renderer: Renderer,
    pub simple_render_system: SimpleRenderSystem,
    pub game_objects: Arc<Mutex<Vec<GameObject>>>,
    pub camera: Arc<Mutex<Camera>>,
}

impl VkApp {
    pub fn new() -> Self {
        let (event_loop, surface, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, surface);
        let simple_render_system = SimpleRenderSystem::new(&renderer);

        let obj_vec = create_game_objects(&renderer);
        let game_objects = Arc::new(Mutex::new(obj_vec));

        let camera = Arc::new(Mutex::new(Camera::new(Some(GameObject::new(None)))));

        Self {
            event_loop,
            renderer, 
            simple_render_system,
            game_objects,
            camera,
        }
    }


    pub fn main_loop(mut self) {
        let mut current_time = Instant::now();

        let mut camera_controller = KeyboardController::new();

        let mut frames: Vec<f32> = vec![];

        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    let delta_time = current_time.elapsed().as_secs_f32();
                    current_time = Instant::now();

                    frames.push(delta_time);
                    if frames.len() > 100 {
                        print!("\rfps: {}", 1.0 / (frames.iter().sum::<f32>() / frames.len() as f32));
                        std::io::stdout().flush().unwrap();
                        frames = vec![];
                    }
                    
                    if let Some((
                        mut builder, 
                        acquire_future, 
                        rebuild_pipeline
                    )) = self.renderer.begin_frame(&self.simple_render_system) {
                        if rebuild_pipeline {
                            self.simple_render_system.pipeline = SimpleRenderSystem::create_pipeline(&self.renderer);
                        }

                        camera_controller.move_xz(delta_time, &mut self.camera.lock().unwrap().object.as_mut().unwrap());
                        self.camera.lock().unwrap().match_obj_transform();

                        let dimensions = self.renderer.swapchain.image_extent();
                        let aspect = dimensions[0] as f32 / dimensions[1] as f32;
                        self.camera.lock().unwrap().set_perspective_projection(50.0_f32.to_radians(), aspect, 0.1, 10.0);

                        animate_game_objects(self.game_objects.clone(), delta_time);
                        builder = self.simple_render_system.render_game_objects(
                            builder, 
                            self.game_objects.clone(),
                            self.camera.clone(),
                        );

                        self.renderer.end_frame(builder, acquire_future);
                    }
                },
                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { input, .. }, ..
                } => {
                    if input.virtual_keycode.is_none() { return };
                    match input.virtual_keycode.unwrap() {
                        VirtualKeyCode::A => camera_controller.move_left = input.state == ElementState::Pressed,
                        VirtualKeyCode::D => camera_controller.move_right = input.state == ElementState::Pressed,
                        VirtualKeyCode::W => camera_controller.move_forward = input.state == ElementState::Pressed,
                        VirtualKeyCode::S => camera_controller.move_backward = input.state == ElementState::Pressed,
                        VirtualKeyCode::Space => camera_controller.move_up = input.state == ElementState::Pressed,
                        VirtualKeyCode::LShift => camera_controller.move_down = input.state == ElementState::Pressed,

                        VirtualKeyCode::Left => camera_controller.look_left = input.state == ElementState::Pressed,
                        VirtualKeyCode::Right => camera_controller.look_right = input.state == ElementState::Pressed,
                        VirtualKeyCode::Up => camera_controller.look_up = input.state == ElementState::Pressed,
                        VirtualKeyCode::Down => camera_controller.look_down = input.state == ElementState::Pressed,
                        _ => {},
                    };
                },
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, ..
                } => {
                    *control_flow = ControlFlow::Exit;
                },
                Event::WindowEvent { 
                    event: WindowEvent::Resized(_), ..
                 } => {
                    self.renderer.recreate_swapchain = true;
                 },
                _ => (),
            }
        });
    }
}