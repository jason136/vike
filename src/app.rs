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
use vulkano::{buffer::CpuBufferPool, swapchain::Surface};
use winit::{
    event::{Event, WindowEvent, ElementState, VirtualKeyCode, DeviceEvent},
    event_loop::{ControlFlow, EventLoop}, window::Window, dpi::LogicalPosition,
};
use std::io::Write;

fn create_game_objects(renderer: &Renderer) -> (HashMap<u32, GameObject>, HashMap<&'static str, Arc<Model>>) {
    let mut game_objects: HashMap<u32, GameObject> = HashMap::new();
    let mut models: HashMap<&str, Arc<Model>> = HashMap::new();

    let basemesh_model = Arc::new(Model::load_obj(renderer, "models/basemesh.obj"));
    let smooth_vase_model = Arc::new(Model::load_obj(renderer, "models/smooth_vase.obj"));
    let flat_vase_model = Arc::new(Model::load_obj(renderer, "models/flat_vase.obj"));
    let floor_model = Arc::new(Model::load_obj(renderer, "models/quad.obj"));

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
        let mut point_light = GameObject::new_point_light(0.5, 0.1, light_colors[i]);

        let rotation = Rotation3::from_axis_angle(
            &Vector3::y_axis(), 
            i as f32 * std::f32::consts::PI * 2.0 / light_colors.len() as f32
        );

        point_light.transform.translation = rotation * Vector3::new(-1.0, -1.0, -1.0);

        game_objects.insert(point_light.id, point_light);
    }

    // for i in 0..1000 {
    //     let mut game_object = GameObject::new(Some(basemesh_model.clone()));
    //     game_object.transform.translation = [0.0, 0.0, 0.0].into();
    //     // game_object.transform.rotation = [std::f32::consts::PI, 0.0, 0.0].into();
    //     // game_object.transform.scale = [0.1; 3].into();

    //     game_object.transform.rotation.y += i as f32 * 0.01;
    //     game_objects.insert(game_object.id, game_object);
    // }

    println!("total verticies: {}", 2184 * 10000 + 5545 + 23894 + 4);

    models.insert("basemesh", basemesh_model);
    models.insert("smooth_vase", smooth_vase_model);
    models.insert("flat_vase", flat_vase_model);
    models.insert("floor", floor_model);
    
    (game_objects, models)
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
        else if obj.id > 2 {
            let rotation = Rotation3::from_axis_angle(
                &Vector3::y_axis(), 
                dt * 0.01
            );

            obj.transform.translation = rotation * obj.transform.translation;
            obj.transform.rotation.y += dt * 0.1;
            // obj.transform.translation.y += dt * 0.01;
        }
    }

    // let mut game_object = GameObject::new(Some(models["basemesh"].clone()));
    // game_object.transform.translation = [4.0, -1.0, 0.0].into();
    // game_object.transform.rotation = [std::f32::consts::PI, 0.0, 0.0].into();
    // game_object.transform.scale = [0.1; 3].into();
    // game_objects.insert(game_object.id, game_object);
}

pub struct VkApp {
    pub event_loop: EventLoop<()>,
    pub window: Arc<Surface<Window>>,
    pub renderer: Renderer,
    pub simple_render_system: StandardRenderSystem,
    pub billboard_render_system: BillboardRenderSystem,
    pub game_objects: HashMap<u32, GameObject>,
    pub models: HashMap<&'static str, Arc<Model>>,
    pub camera: Camera,
    pub uniform_buffer: CpuBufferPool<vs::ty::UniformBufferData>,
}

impl VkApp {
    pub fn new() -> Self {
        let (event_loop, window, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, window.clone());

        let simple_render_system = StandardRenderSystem::new(&renderer);
        let billboard_render_system = BillboardRenderSystem::new(&renderer);

        let (game_objects, models) = create_game_objects(&renderer);

        let mut camera_object = GameObject::new(None);
        camera_object.transform.translation.z = -2.5; 
        let camera = Camera::new(Some(camera_object));

        let uniform_buffer = CpuBufferPool::<vs::ty::UniformBufferData>::uniform_buffer(renderer.device.clone());

        Self {
            event_loop,
            window,
            renderer, 
            uniform_buffer,
            simple_render_system,
            billboard_render_system,
            game_objects,
            models,
            camera,
        }
    }

    pub fn main_loop(mut self) {
        let mut current_time = Instant::now();

        let mut camera_controller = KeyboardController::new();

        let mut frames: Vec<f32> = vec![];
        let mut frame_count = 0;

        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    let delta_time = current_time.elapsed().as_secs_f32();
                    current_time = Instant::now();

                    frames.push(delta_time);
                    if frames.len() > 100 {
                        // print!("\rfps: {} total verticies: {}", 1.0 / (frames.iter().sum::<f32>() / frames.len() as f32), frame_count * 2184 + 5545 + 23894 + 4);
                        print!("\rfps: {}", 1.0 / (frames.iter().sum::<f32>() / frames.len() as f32));
                        std::io::stdout().flush().unwrap();
                        frames = vec![];
                    }
                    frame_count += 1;

                    camera_controller.move_xz(delta_time, &mut self.camera.object.as_mut().unwrap());
                    if camera_controller.mouse_engaged {
                        self.window.window().set_cursor_visible(false);
                        self.window.window().set_cursor_position(LogicalPosition::new(100, 100)).unwrap();
                    }
                    else {
                        self.window.window().set_cursor_visible(true);
                    }

                    self.camera.match_obj_transform();

                    let dimensions = self.renderer.swapchain.image_extent();
                    let aspect = dimensions[0] as f32 / dimensions[1] as f32;
                    self.camera.set_perspective_projection(50.0_f32.to_radians(), aspect, 0.1, 500.0);

                    animate_game_objects(&mut self.game_objects, delta_time);
                    
                    let uniform_buffer_subbuffer = {
                        let (num_lights, point_lights) = self.billboard_render_system.update_point_lights(&self.game_objects);

                        let uniform_data = vs::ty::UniformBufferData {
                            projection: self.camera.projection_matrix.into(),
                            view: self.camera.view_matrix.into(),
                            inverseView: self.camera.inverse_view_matrix.into(),
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
                            &self.game_objects,
                        );
                        builder = self.billboard_render_system.render(
                            builder, 
                            uniform_buffer_subbuffer,
                            &self.game_objects,
                            &self.camera,
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

                        VirtualKeyCode::Escape => camera_controller.disable_mouse_engaged = input.state == ElementState::Pressed,
                        _ => {},
                    };
                },
                Event::DeviceEvent { 
                    event: DeviceEvent::MouseMotion { delta }, ..
                } => {
                    camera_controller.mouse_delta = delta;
                },
                Event::DeviceEvent { 
                    event: DeviceEvent::Button { button, state }, ..
                } => {
                    if button == 1 && state == ElementState::Pressed && camera_controller.cursor_in_window {
                        camera_controller.focused = true;
                    }
                },
                Event::WindowEvent { 
                    event: WindowEvent::Focused(focused), ..
                } => {
                    camera_controller.focused = focused;
                }
                Event::WindowEvent { 
                    event: WindowEvent::CursorEntered { .. }, ..
                } => {
                    camera_controller.cursor_in_window = true;
                }
                Event::WindowEvent { 
                    event: WindowEvent::CursorLeft { .. }, ..
                } => {
                    camera_controller.cursor_in_window = false;
                }
                // Event::WindowEvent { 
                //     event: WindowEvent::(focused), ..
                // } => {
                //     camera_controller.cursor_in_window = false;
                // }
                Event::WindowEvent { 
                    event: WindowEvent::Resized(_), ..
                } => {
                    self.renderer.recreate_swapchain = true;
                },
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, ..
                } => {
                    *control_flow = ControlFlow::Exit;
                },
                _ => (),
            }
        });
    }
}