use crate::{
    simple_render_system::{SimpleRenderSystem, Vertex},
    renderer::Renderer,
    game_object::{self, GameObject},
};

use std::sync::{Arc, Mutex};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

fn animate_game_objects(
    game_objects: Arc<Mutex<Vec<game_object::GameObject>>>, 
) {
    for obj in game_objects.lock().unwrap().iter_mut() {
        obj.transform.rotation.y += 0.01 * std::f32::consts::PI * 2.0;
        obj.transform.rotation.x += 0.005 * std::f32::consts::PI * 2.0;
    }
}

fn cube_model(renderer: &Renderer, offset: [f32; 3]) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
    let mut vertices: Vec<Vertex> = vec![
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [-0.5, 0.5, 0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [-0.5, -0.5, 0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [-0.5, 0.5, -0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [-0.5, 0.5, 0.5], color: [0.9, 0.9, 0.9] }, 
        Vertex { position: [0.5, -0.5, -0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [0.5, -0.5, 0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [0.5, -0.5, -0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [0.5, 0.5, -0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.8, 0.8, 0.1] }, 
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [0.5, -0.5, 0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [-0.5, -0.5, 0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [0.5, -0.5, -0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [0.5, -0.5, 0.5], color: [0.9, 0.6, 0.1] }, 
        Vertex { position: [-0.5, 0.5, -0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [-0.5, 0.5, 0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [-0.5, 0.5, -0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [0.5, 0.5, -0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.8, 0.1, 0.1] }, 
        Vertex { position: [-0.5, -0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [-0.5, 0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [-0.5, -0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [0.5, -0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [0.5, 0.5, 0.5], color: [0.1, 0.1, 0.8] }, 
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
        Vertex { position: [0.5, 0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
        Vertex { position: [-0.5, 0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
        Vertex { position: [-0.5, -0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
        Vertex { position: [0.5, -0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
        Vertex { position: [0.5, 0.5, -0.5], color: [0.1, 0.8, 0.1] }, 
    ];

    for i in 0..vertices.len() { 
        vertices[i].position[0] += offset[0];
        vertices[i].position[1] += offset[1];
        vertices[i].position[2] += offset[2];
    };

    let vertex_buffer = CpuAccessibleBuffer::from_iter(renderer.device.clone(), BufferUsage::vertex_buffer(), false, vertices)
        .expect("Failed to create vertex buffer");

    vertex_buffer
}

fn create_game_objects(renderer: &Renderer) -> Vec<game_object::GameObject> {
    let cube_model = cube_model(renderer, [0.0, 0.0, 0.0]);

    let mut game_objects = vec![];

    let mut cube = GameObject::new(cube_model);
    cube.transform.translation = [0.0, 0.0, 0.5].into();
    cube.transform.scale = [0.5, 0.5, 0.5].into();
    game_objects.push(cube);
    
    game_objects
}

pub struct FkApp {
    pub event_loop: EventLoop<()>,
    pub renderer: Renderer,
    pub simple_render_system: SimpleRenderSystem,
    pub game_objects: Arc<Mutex<Vec<game_object::GameObject>>>,
}

impl FkApp {
    pub fn new() -> Self {
        let (event_loop, surface, instance) = Renderer::create_window();
        let renderer = Renderer::new(instance, surface);
        let simple_render_system = SimpleRenderSystem::new(&renderer);

        let obj_vec = create_game_objects(&renderer);
        let game_objects = Arc::new(Mutex::new(obj_vec));

        Self {
            event_loop,
            renderer, 
            simple_render_system,
            game_objects,
        }
    }

    pub fn main_loop(mut self) {
        self.event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested, 
                    ..
                } => {
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent { 
                    event: WindowEvent::Resized(_), 
                    ..
                 } => {
                    self.renderer.recreate_swapchain = true;
                 }
                Event::RedrawEventsCleared => {
                    
                    if let Some((mut builder, acquire_future)) = self.renderer.begin_frame(&self.simple_render_system) {

                        animate_game_objects(self.game_objects.clone());
                        builder = self.simple_render_system.render_game_objects(builder, self.game_objects.clone());

                        self.renderer.end_frame(builder, acquire_future);
                    }
                }
                _ => (),
            }
        });
    }
}