use crate::game_object::Transform3D;
use nalgebra::{Matrix4, Vector3};

pub struct Camera {
    pub transform: Option<Transform3D>,
    pub projection_matrix: Matrix4<f32>,
    pub view_matrix: Matrix4<f32>,
    pub inverse_view_matrix: Matrix4<f32>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        CameraUniform {
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {

        #[rustfmt::skip]
        let OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::from_column_slice(&[
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.5,
            0.0, 0.0, 0.0, 1.0,
        ]);

        self.view_proj =
            (OPENGL_TO_WGPU_MATRIX * camera.projection_matrix * camera.view_matrix).into();
    }
}

#[rustfmt::skip]
impl Camera {
    pub fn new(transform: Option<Transform3D>) -> Self {
        Camera {
            transform,
            projection_matrix: Matrix4::identity(),
            view_matrix: Matrix4::identity(),
            inverse_view_matrix: Matrix4::identity(),
        }
    }

    pub fn set_orthographic_projection(&mut self, 
        left: f32, right: f32, 
        top: f32, bottom: f32, 
        near: f32, far: f32,
    ) { 
        self.projection_matrix = Matrix4::from_column_slice(&[
            2.0 / (right - left), 0.0, 0.0, 0.0,

            0.0, 2.0 / (bottom - top), 0.0, 0.0,

            0.0, 0.0, 1.0 / (far - near), 0.0,

            -(right + left) / (right - left), 
            -(bottom + top) / (bottom - top), 
            -near / (far - near), 
            1.0,
        ]);
    }

    pub fn set_perspective_projection(&mut self, 
        fovy: f32, aspect: f32, 
        near: f32, far: f32,
    ) {
        let tan_half_fovy = (fovy / 2.0).tan();
        self.projection_matrix = Matrix4::from_column_slice(&[
            1.0 / (aspect * tan_half_fovy), 0.0, 0.0, 0.0,

            0.0, 1.0 / (tan_half_fovy), 0.0, 0.0,

            0.0, 0.0, far / (far - near), 1.0, 

            0.0, 0.0, -(far * near) / (far - near), 0.0,
        ]);
    }

    pub fn set_view_direction(&mut self, position: Vector3<f32>, direction: Vector3<f32>, up: Vector3<f32>) {
        let w = direction.normalize();
        let u = w.cross(&up).normalize();
        let v = w.cross(&u);

        self.view_matrix = Matrix4::from_column_slice(&[
            u.x, v.x, w.x, 0.0, 
            u.y, v.y, w.y, 0.0, 
            u.z, v.z, w.z, 0.0, 
            -u.dot(&position), -v.dot(&position), -w.dot(&position), 1.0,
        ]);

        self.inverse_view_matrix = Matrix4::from_column_slice(&[
            u.x, u.y, u.z, 0.0, 
            v.x, v.y, v.z, 0.0, 
            w.x, w.y, w.z, 0.0, 
            position.x, position.y, position.z, 1.0,
        ]);
    }

    pub fn set_view_target(&mut self, position: Vector3<f32>, target: Vector3<f32>, up: Vector3<f32>) {
        Camera::set_view_direction(self, position, target - position, up);
    }

    pub fn match_transform(&mut self) {
        if let Some(transform) = &self.transform {
            self.set_view_xyz(transform.translation, transform.rotation);
        }
    }

    pub fn set_view_xyz(&mut self, position: Vector3<f32>, rotation: Vector3<f32>) {
        let c3 = rotation.z.cos();
        let s3 = rotation.z.sin();
        let c2 = rotation.x.cos();
        let s2 = rotation.x.sin();
        let c1 = rotation.y.cos();
        let s1 = rotation.y.sin();
        let u = Vector3::new(
            c1 * c3 + s1 * s2 * s3, c2 * s3, c1 * s2 * s3 - c3 * s1,
        );
        let v = Vector3::new(
            c3 * s1 * s2 - c1 * s3, c2 * c3, c1 * c3 * s2 + s1 * s3,
        );
        let w = Vector3::new(
            c2 * s1, -s2, c1 * c2,
        );

        self.view_matrix = Matrix4::from_column_slice(&[
            u.x, v.x, w.x, 0.0, 
            u.y, v.y, w.y, 0.0, 
            u.z, v.z, w.z, 0.0, 
            -u.dot(&position), -v.dot(&position), -w.dot(&position), 1.0,
        ]);
        
        self.inverse_view_matrix = Matrix4::from_column_slice(&[
            u.x, u.y, u.z, 0.0, 
            v.x, v.y, v.z, 0.0, 
            w.x, w.y, w.z, 0.0, 
            position.x, position.y, position.z, 1.0,
        ]);
    }
}
