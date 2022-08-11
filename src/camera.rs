use nalgebra::Matrix4;

pub struct Camera {
    pub projection_matrix: Matrix4<f32>,
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            projection_matrix: Matrix4::identity(),
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
}