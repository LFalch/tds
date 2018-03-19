use ggez::{Context, GameResult};
use ggez::graphics::{self, Point2, Vector2, Image};
// use ggez::nalgebra as na;

use game::world::Grid;
use game::DELTA;

#[derive(Debug, Clone, Serialize, Deserialize,)]
/// A simple object that can be drawn to the screen
pub struct Object {
    #[serde(serialize_with = "::save::point_ser", deserialize_with = "::save::point_des")]
    /// The position of the object
    pub pos: Point2,
    /// The rotation of the obejct in radians
    pub rot: f32,
}

impl Object {
    /// Make a new physics object
    pub fn new(pos: Point2) -> Self {
        Object {
            pos,
            rot: 0.,
        }
    }
    /// Draw the object
    pub fn draw(&self, ctx: &mut Context, img: &Image) -> GameResult<()> {
        let drawparams = graphics::DrawParam {
            dest: self.pos,
            rotation: self.rot,
            offset: Point2::new(0.5, 0.5),
            .. Default::default()
        };
        graphics::draw_ex(ctx, img, drawparams)
    }
    pub fn is_on_solid(&self, grid: &Grid) -> bool {
        let (x, y) = Grid::snap(self.pos);
        grid.get(x, y).solid()
    }
    pub fn move_on_grid(&mut self, mut v: Vector2, speed: f32, grid: &Grid) {
        if v.x != 0. {
            let (xx, xy) = Grid::snap(self.pos + Vector2::new(16. * v.x, 0.));
            if grid.get(xx, xy).solid() {
                v.x = 0.;
            }
        }
        if v.y != 0. {
            let (yx, yy) = Grid::snap(self.pos + Vector2::new(0., 16. * v.y));
            if grid.get(yx, yy).solid() {
                v.y = 0.;
            }
        }

        if v.norm_squared() != 0. {
            v = v.normalize();
        }
        self.pos += v * speed * DELTA;
    }
}