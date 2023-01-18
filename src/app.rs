use crate::{
    render_systems::standard_render_system::{StandardRenderSystem, vs},
    render_systems::billboard_render_system::BillboardRenderSystem,
    renderer::Renderer,
    game_object::{GameObject, Model},
    camera::Camera,
    movement::KeyboardController,
};

use std::{sync::Arc, time::Instant, collections::HashMap};
use nalgebra::{Vector3, Rotation3};
use vulkano::buffer::CpuBufferPool;
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

    let light_colors = vec![
        Vector3::new(1.0, 0.1, 0.1),
        Vector3::new(0.1, 0.1, 1.0),
        Vector3::new(0.1, 1.0, 0.1),
        Vector3::new(1.0, 1.0, 0.1),
        Vector3::new(0.1, 1.0, 1.0),
        Vector3::new(1.0, 1.0, 1.0),
    ];

    for i in 0..light_colors.len() {
        let mut point_light = GameObject::new_point_light(0.2, 0.1, light_colors[i]);

        let rotation = Rotation3::from_axis_angle(
            &Vector3::y_axis(), 
            i as f32 * std::f32::consts::PI * 2.0 / light_colors.len() as f32
        );

        point_light.transform.translation = rotation * Vector3::new(-1.0, -1.0, -1.0);

        game_objects.insert(point_light.id, point_light);
    }
    
    game_objects
}

fn animate_game_objects(game_objects: &mut HashMap<u32, GameObject>, dt: f32) {
    for obj in game_objects.values_mut() {
        if obj.point_light.is_some() {
            let rotation = Rotation3::from_axis_angle(
                &Vector3::y_axis(), 
                dt
            );
    
            obj.transform.translation = rotation * obj.transform.translation;    
        }
    }
}

pub struct VkApp {
    pub event_loop: EventLoop<()>,
    pub renderer: Renderer,
    pub simple_render_system: StandardRenderSystem,
    pub billboard_render_system: BillboardRenderSystem,
    pub game_objects: HashMap<u32, GameObject>,
    pub camera: Camera,
    pub uniform_buffer: CpuBufferPool<vs::ty::UniformBufferData>,
}

impl VkApp {
    pub fn new() -> Self {
        let (event_loop, surface, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, surface);

        let simple_render_system = StandardRenderSystem::new(&renderer);
        let billboard_render_system = BillboardRenderSystem::new(&renderer);

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
            billboard_render_system,
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

                    animate_game_objects(&mut self.game_objects, delta_time);

                    camera_controller.move_xz(delta_time, &mut self.camera.object.as_mut().unwrap());
                    self.camera.match_obj_transform();

                    let dimensions = self.renderer.swapchain.image_extent();
                    let aspect = dimensions[0] as f32 / dimensions[1] as f32;
                    self.camera.set_perspective_projection(50.0_f32.to_radians(), aspect, 0.1, 500.0);
                    
                    let uniform_buffer_subbuffer = {
                        let (num_lights, point_lights) = self.billboard_render_system.update_point_lights(self.game_objects.clone());

                        let uniform_data = vs::ty::UniformBufferData {
                            projection: self.camera.projection_matrix.into(),
                            view: self.camera.view_matrix.into(),
                            ambientLightColor: [1.0, 1.0, 1.0, 0.02].into(),
                            pointLights: point_lights,
                            numLights: num_lights,
                        };

                        self.uniform_buffer.next(uniform_data).unwrap()
                    };

                    if let Some((mut builder, acquire_future, rebuild_pipeline)) = self.renderer.begin_frame() {
                        if rebuild_pipeline {
                            self.simple_render_system.pipeline = StandardRenderSystem::create_pipeline(&self.renderer);
                            self.billboard_render_system.pipeline = BillboardRenderSystem::create_pipeline(&self.renderer);
                        }

                        builder = self.simple_render_system.render_game_objects(
                            builder, 
                            uniform_buffer_subbuffer.clone(),
                            self.game_objects.clone(),
                        );
                        builder = self.billboard_render_system.render(
                            builder, 
                            uniform_buffer_subbuffer,
                            self.game_objects.clone(),
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