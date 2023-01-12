use crate::{
    render_systems::simple_render_system::{SimpleRenderSystem, vs},
    render_systems::billboard_render_system::PointLightSystem,
    renderer::Renderer,
    game_object::{GameObject, Model},
    camera::Camera,
    movement::KeyboardController,
};

use std::{sync::{Arc, Mutex}, time::Instant, collections::HashMap};
use vulkano::{buffer::CpuBufferPool, descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet}, pipeline::PipelineBindPoint};
use vulkano::pipeline::Pipeline;
use winit::{
    event::{Event, WindowEvent, ElementState, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
};
use std::io::Write;

fn create_game_objects(renderer: &Renderer) -> HashMap<u32, GameObject> {
    let smooth_vase_model = Arc::new(Model::load_obj(renderer, "models/smooth_vase.obj"));
    let flat_vase_model = Arc::new(Model::load_obj(renderer, "models/flat_vase.obj"));
    let floor_model = Arc::new(Model::load_obj(renderer, "models/quad.obj"));

    let mut game_objects: HashMap<u32, GameObject> = HashMap::new();

    let mut game_object = GameObject::new(Some(smooth_vase_model.clone()));
    game_object.transform.translation = [-0.5, 0.5, 0.0].into();
    game_object.transform.scale = [2.0; 3].into();
    game_objects.insert(game_object.id, game_object);

    let mut game_object = GameObject::new(Some(flat_vase_model.clone()));
    game_object.transform.translation = [0.5, 0.5, 0.0].into();
    game_object.transform.scale = [2.0; 3].into();
    game_objects.insert(game_object.id, game_object);

    let mut game_object = GameObject::new(Some(floor_model.clone()));
    game_object.transform.translation = [0.0, 0.5, 0.0].into();
    game_object.transform.scale = [3.0, 1.0, 3.0].into();
    game_objects.insert(game_object.id, game_object);
    
    game_objects
}

fn animate_game_objects(game_objects: HashMap<u32, GameObject>, dt: f32) {
    // for obj in game_objects.lock().unwrap().iter_mut() {
    //     obj.transform.rotation.y += 1.0 * dt * std::f32::consts::PI * 2.0;
    //     obj.transform.rotation.x += 0.5 * dt * std::f32::consts::PI * 2.0;
    // }
}

pub struct VkApp {
    pub event_loop: EventLoop<()>,
    pub renderer: Renderer,
    pub simple_render_system: SimpleRenderSystem,
    pub billboard_system: PointLightSystem,
    pub game_objects: HashMap<u32, GameObject>,
    pub camera: Camera,
    pub uniform_buffer: CpuBufferPool<vs::ty::UniformBufferData>,
}

impl VkApp {
    pub fn new() -> Self {
        let (event_loop, surface, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, surface);

        let simple_render_system = SimpleRenderSystem::new(&renderer);
        let billboard_system = PointLightSystem::new(&renderer);

        let game_objects = create_game_objects(&renderer);

        let mut camera_object = GameObject::new(None);
        camera_object.transform.translation.z = -2.5; 
        let camera = Camera::new(Some(camera_object));

        let uniform_buffer = CpuBufferPool::<vs::ty::UniformBufferData>::uniform_buffer(renderer.device.clone());

        Self {
            event_loop,
            renderer, 
            uniform_buffer,
            simple_render_system,
            billboard_system,
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

                    camera_controller.move_xz(delta_time, &mut self.camera.object.as_mut().unwrap());
                    self.camera.match_obj_transform();

                    let dimensions = self.renderer.swapchain.image_extent();
                    let aspect = dimensions[0] as f32 / dimensions[1] as f32;
                    self.camera.set_perspective_projection(50.0_f32.to_radians(), aspect, 0.1, 500.0);
                    
                    if let Some((mut builder, acquire_future, rebuild_pipeline)) = self.renderer.begin_frame() {
                        if rebuild_pipeline {
                            self.simple_render_system.pipeline = SimpleRenderSystem::create_pipeline(&self.renderer);
                        }

                        let uniform_buffer_subbuffer = {
                            let uniform_data = vs::ty::UniformBufferData {
                                projection: self.camera.projection_matrix.into(),
                                view: self.camera.view_matrix.into(),
                                ambientLightColor: [1.0, 1.0, 1.0, 0.02].into(),
                                lightPosition: [-1.0, -1.0, -1.0].into(),
                                lightColor: [1.0; 4].into(),
                                _dummy0: [0; 4],
                            };
                            self.uniform_buffer.next(uniform_data).unwrap()
                        };

                        let layout = self.simple_render_system.pipeline.layout().set_layouts().get(0).unwrap();
                        let set = PersistentDescriptorSet::new(
                            layout.clone(), 
                            [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
                        ).unwrap();
                        builder.bind_descriptor_sets(
                            PipelineBindPoint::Graphics, 
                            self.simple_render_system.pipeline.layout().clone(), 
                            0,
                            set.clone(), 
                        );

                        animate_game_objects(self.game_objects.clone(), delta_time);

                        builder = self.simple_render_system.render_game_objects(
                            builder, 
                            self.game_objects.clone(),
                        );
                        // builder = self.billboard_system.render(builder);
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