use crate::simple_render_system::Vertex;

use vulkano::buffer::CpuAccessibleBuffer;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use nalgebra::{Matrix4, Vector3};

#[derive(Clone)]
pub struct Transform3D {
    pub translation: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub rotation: Vector3<f32>,
}

impl Transform3D {
    pub fn mat4(&self) -> Matrix4<f32> {
        let c3 = self.rotation.z.cos();
        let s3 = self.rotation.z.sin();
        let c2 = self.rotation.x.cos();
        let s2 = self.rotation.x.sin();
        let c1 = self.rotation.y.cos();
        let s1 = self.rotation.y.sin();

        Matrix4::from_column_slice(&[
            self.scale.x * (c1 * c3 + s1 * s2 * s3),
            self.scale.x * (c2 * s3),
            self.scale.x * (c1 * s2 * s3 - c3 * s1),
            0.0,

            self.scale.y * (c3 * s1 * s2 - c1 * s3),
            self.scale.y * (c2 * c3),
            self.scale.y * (c1 * c3 * s2 + s1 * s3),
            0.0,

            self.scale.z * (c2 * s1),
            self.scale.z * (-s2),
            self.scale.z * (c1 * c2),
            0.0,

            self.translation.x, 
            self.translation.y, 
            self.translation.z, 
            1.0,
        ])
    }
}

static COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone)]
pub struct GameObject {
    pub id: u32,
    pub transform: Transform3D,
    pub color: [f32; 3],
    pub model: Arc<CpuAccessibleBuffer<[Vertex]>>,
}

impl GameObject {
    pub fn new(model: Arc<CpuAccessibleBuffer<[Vertex]>>) -> GameObject {
        let id = COUNT.load(Ordering::SeqCst);
        COUNT.fetch_add(1, Ordering::SeqCst);

        let transform = Transform3D {
            translation: Vector3::new(0.0, 0.0, 0.0),
            scale: Vector3::new(1.0, 1.0, 1.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
        };

        GameObject {
            id,
            transform,
            color: [0.0, 0.0, 0.0],
            model,
        }
    }
}