use nalgebra::{Vector3, clamp};

use crate::game_object::GameObject;

pub struct KeyboardController {
    pub move_left: bool,
    pub move_right: bool,
    pub move_forward: bool,
    pub move_backward: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub look_left: bool,
    pub look_right: bool,
    pub look_up: bool,
    pub look_down: bool,
    pub disable_mouse_engaged: bool,
    pub focused: bool,
    pub cursor_in_window: bool,
    pub occluded: bool,

    pub focused_previous: bool,
    pub mouse_engaged: bool,
    pub mouse_delta: (f64, f64),

    pub move_speed: f32, 
    pub look_speed: f32,
}

impl KeyboardController {
    pub fn new() -> Self {
        Self {
            move_left: false,
            move_right: false,
            move_forward: false,
            move_backward: false,
            move_up: false,
            move_down: false,
            look_left: false,
            look_right: false,
            look_up: false,
            look_down: false,
            disable_mouse_engaged: false,
            focused: false,
            cursor_in_window: false,
            occluded: false,

            focused_previous: false,
            mouse_engaged: false,
            mouse_delta: (0.0, 0.0),

            move_speed: 3.0,
            look_speed: 2.0,
        }
    }

    pub fn move_xz(&mut self, dt: f32, game_object: &mut GameObject) {
        let mut rotate = Vector3::new(0.0, 0.0, 0.0);
        if self.look_right { rotate.y += 1.0; }
        if self.look_left { rotate.y -= 1.0; }
        if self.look_up { rotate.x += 1.0; }
        if self.look_down { rotate.x -= 1.0; }
 
        if self.focused != self.focused_previous { 
            println!("setting focused to {}", self.focused);
            self.focused_previous = self.focused;
            self.mouse_engaged = self.focused;
        }
        if self.disable_mouse_engaged { self.mouse_engaged = false; }

        if self.mouse_engaged {
            rotate.x -= self.mouse_delta.1 as f32 * self.look_speed;
            rotate.y += self.mouse_delta.0 as f32 * self.look_speed;    
        }
        self.mouse_delta = (0.0, 0.0);

        if rotate.dot(&rotate) > 0.0 {
            game_object.transform.rotation += self.look_speed * dt * rotate.normalize();
        }
        game_object.transform.rotation.x = clamp(game_object.transform.rotation.x, -1.5, 1.5);
        game_object.transform.rotation.y = game_object.transform.rotation.y % (std::f32::consts::PI * 2.0);

        let yaw = game_object.transform.rotation.y;
        let forward_direction = Vector3::new(yaw.sin(), 0.0, yaw.cos());
        let right_direction = Vector3::new(forward_direction.z, 0.0, -forward_direction.x);
        let up_direction = Vector3::new(0.0, -1.0, 0.0);

        let mut move_direction = Vector3::new(0.0, 0.0, 0.0);
        if self.move_forward { move_direction += forward_direction; }
        if self.move_backward { move_direction -= forward_direction; }
        if self.move_left { move_direction -= right_direction; }
        if self.move_right { move_direction += right_direction; }
        if self.move_up { move_direction += up_direction; }
        if self.move_down { move_direction -= up_direction; }

        if move_direction.dot(&move_direction) > 0.0 {
            game_object.transform.translation += self.move_speed * dt * move_direction.normalize();
        }
    }
}