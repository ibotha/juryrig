use super::buffer::Buffer;

pub struct Camera {
    pub(super) viewmatrix: na::Matrix4<f32>,
    pub(super) position: na::Vector3<f32>,
    view_direction: na::Unit<na::Vector3<f32>>,
    down_direction: na::Unit<na::Vector3<f32>>,

    fovy: f32,
    pub(super) aspect: f32,
    near: f32,
    far: f32,
    pub(super) projectionmatrix: na::Matrix4<f32>,
}
impl Default for Camera {
    fn default() -> Self {
        let mut cam = Camera {
            viewmatrix: na::Matrix4::identity(),
            position: na::Vector3::new(0.0, 0.0, 0.0),
            view_direction: na::Unit::new_normalize(na::Vector3::new(0.0, 0.0, 1.0)),
            down_direction: na::Unit::new_normalize(na::Vector3::new(0.0, -1.0, 0.0)),
            fovy: std::f32::consts::FRAC_PI_3,
            aspect: 800.0 / 600.0,
            near: 0.1,
            far: 100.0,
            projectionmatrix: na::Matrix4::identity(),
        };
        cam.update_projectionmatrix();
        cam.update_viewmatrix();
        cam
    }
}
impl Camera {
    pub fn update_viewmatrix(&mut self) {
        let right = na::Unit::new_normalize(self.down_direction.cross(&self.view_direction));
        let m = na::Matrix4::new(
            right.x,
            right.y,
            right.z,
            -right.dot(&self.position), //
            self.down_direction.x,
            self.down_direction.y,
            self.down_direction.z,
            -self.down_direction.dot(&self.position), //
            self.view_direction.x,
            self.view_direction.y,
            self.view_direction.z,
            -self.view_direction.dot(&self.position), //
            0.0,
            0.0,
            0.0,
            1.0,
        );
        self.viewmatrix = m;
    }
    pub fn update_projectionmatrix(&mut self) {
        let d = 1.0 / (0.5 * self.fovy).tan();
        self.projectionmatrix = na::Matrix4::new(
            d / self.aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            d,
            0.0,
            0.0,
            0.0,
            0.0,
            self.far / (self.far - self.near),
            -self.near * self.far / (self.far - self.near),
            0.0,
            0.0,
            1.0,
            0.0,
        );
    }
    pub fn move_forward(&mut self, distance: f32) {
        self.position += distance * self.view_direction.as_ref();
        self.update_viewmatrix();
    }
    pub fn move_backward(&mut self, distance: f32) {
        self.move_forward(-distance);
    }
    pub fn turn_right(&mut self, angle: f32) {
        let rotation = na::Rotation3::from_axis_angle(&self.down_direction, angle);
        self.view_direction = rotation * self.view_direction;
        self.update_viewmatrix();
    }
    pub fn turn_left(&mut self, angle: f32) {
        self.turn_right(-angle);
    }
    pub fn turn_up(&mut self, angle: f32) {
        let right = na::Unit::new_normalize(self.down_direction.cross(&self.view_direction));
        let rotation = na::Rotation3::from_axis_angle(&right, angle);
        self.view_direction = rotation * self.view_direction;
        self.down_direction = rotation * self.down_direction;
        self.update_viewmatrix();
    }
    pub fn turn_down(&mut self, angle: f32) {
        self.turn_up(-angle);
    }
}
