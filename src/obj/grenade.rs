use ggez::{Context, GameResult, graphics::{self, WHITE, Color, Mesh, DrawParam}, input::{mouse}};
use std::{iter, f32::consts::{PI, FRAC_PI_2 as HALF_PI}};
use rand::{thread_rng, Rng};

const PI_MUL_2: f32 = 2. * PI;

use crate::{
    util::{angle_to_vec, Vector2},
    game::{
        DELTA,
        world::{Grid, Palette},
        event::MouseButton,
    },
    io::{
        snd::MediaPlayer,
        tex::{Assets, },
    },
};
use super::{Object, player::Player, enemy::Enemy, health::Health};

#[derive(Debug, Default, Clone, Copy)]
pub struct Utilities {
    pub grenades: u8,
}

#[derive(Debug, Clone)]
pub struct Grenade {
    pub obj: Object,
    pub vel: Vector2,
    pub state: GrenadeState,
}

#[derive(Debug, Clone)]
pub enum GrenadeState {
    Cooked {
        fuse: f32,
    },
    Fused {
        fuse: f32,
    },
    Explosion {
        alive_time: f32,
        mesh: Mesh,
    }

}

const EXPLOSION_LIFETIME: f32 = 0.5;
const DEC: f32 = 1.4;

const RANGE: f32 = 144.;
const LETHAL_RANGE: f32 = 64.;

impl Grenade {
    #[inline]
    pub fn apply_damage(health: &mut Health, high: bool) {
        health.weapon_damage(if high { 105.} else {55.}, 0.85);
    }
    #[inline]
    pub fn draw(&self, ctx: &mut Context, a: &Assets) -> GameResult<()> {
        match &self.state {
            GrenadeState::Cooked{..} => {
                let img = a.get_img(ctx, "weapons/pineapple");
                self.obj.draw(ctx, &*img, WHITE)
            }
            GrenadeState::Fused{..} => {
                let img = a.get_img(ctx, "weapons/pineapple");
                self.obj.draw(ctx, &*img, WHITE)
            }
            GrenadeState::Explosion { mesh, alive_time } => {
                const EXPANDING_TIME: f32 = 0.1;
                let mut dp = DrawParam::from((self.obj.pos,));

                if *alive_time <= EXPANDING_TIME {
                    let scale = alive_time / EXPANDING_TIME;
                    dp = dp.scale(Vector2::new(scale, scale));
                } else {
                    let colour = (HALF_PI * (alive_time - EXPANDING_TIME) / (EXPLOSION_LIFETIME - EXPANDING_TIME)).cos();
                    dp = dp.color(Color{r: colour, g: colour, b: colour, a: 0.5+0.5*colour});
                }

                graphics::draw(ctx, mesh, dp)
            }
        }
    }
    fn make_mesh(&self, ctx: &mut Context, a: &Assets, palette: &Palette, grid: &Grid) -> GameResult<Mesh> {
        const NUM_VERTICES: u32 = 120;
        const RADIANS_PER_VERT: f32 = (360. / NUM_VERTICES as f32) * PI/180.;

        let random_offset = thread_rng().gen_range(0., PI_MUL_2);

        let centre = graphics::Vertex {
            pos: [0., 0.],
            uv: [0.5, 0.5],
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let vertices: Vec<_> = (0..NUM_VERTICES).map(|i| {
            let angle = RANGE * angle_to_vec(i as f32 * RADIANS_PER_VERT);
            let angle_uv = 0.5 * angle_to_vec(i as f32 * RADIANS_PER_VERT + random_offset);
            let cast = grid.ray_cast(palette, self.obj.pos, angle, true);
            graphics::Vertex{
                pos: (cast.into_point() - self.obj.pos).into(),
                uv: (Vector2::new(0.5, 0.5) + (cast.clip().norm()-RANGE)/RANGE * angle_uv).into(),
                color: [1.0, 1.0, 1.0, 1.0],
            }
        }).chain(iter::once(centre)).collect();
        
        let indices = (0..NUM_VERTICES).flat_map(|i| iter::once(NUM_VERTICES).chain(iter::once(i)).chain(iter::once((i + 1) % NUM_VERTICES))).collect::<Vec<_>>();
        let expl_img = (a.get_img(ctx, "weapons/explosion1")).clone();
        Mesh::from_raw(ctx, &vertices, &indices, Some(expl_img))
    }
    pub fn fuse_update(obj: &Object, fuse: &mut f32, palette: &Palette, grid: &Grid, player: &mut Player, enemies: &mut [Enemy]) -> GrenadeUpdate {
        if *fuse > DELTA {
            *fuse -= DELTA;
            GrenadeUpdate::None
        } else {
            *fuse = 0.;
            Self::explode(obj, palette, grid, player, enemies)
        }
    }
    pub fn explode(obj: &Object, palette: &Palette, grid: &Grid, player: &mut Player, enemies: &mut [Enemy]) -> GrenadeUpdate {
        let start = obj.pos;

        let player_hit;
        let mut enemy_hits = Vec::new();
        
        let d_player = player.obj.pos-start;
        if d_player.norm() < RANGE && grid.ray_cast(palette, start, d_player, true).full() {
            Self::apply_damage(&mut player.health, d_player.norm() <= LETHAL_RANGE);
            player_hit = true;
        } else {
            player_hit = false;
        }
        
        for (i, enem) in enemies.iter_mut().enumerate().rev() {
            let d_enemy = enem.pl.obj.pos - start;
            if d_enemy.norm() < 144. && grid.ray_cast(palette, start, d_enemy, true).full() {
                Self::apply_damage(&mut enem.pl.health, d_enemy.norm() <= 64.);
                enemy_hits.push(i);
            }
        }   
        GrenadeUpdate::Explosion{player_hit, enemy_hits}
    }
    pub fn update_fused(obj: &mut Object, vel: &mut Vector2, fuse: &mut f32, palette: &Palette, grid: &Grid, player: &mut Player, enemies: &mut [Enemy]) -> GrenadeUpdate {
        let start = obj.pos;
        let d_vel = -DEC * (*vel) * DELTA;
        let d_pos = 0.5 * DELTA * d_vel + (*vel) * DELTA;
        *vel += d_vel;
        let change_fuse = Self::fuse_update(obj, fuse, palette, grid, player, enemies);
        if change_fuse != GrenadeUpdate::None {
            return change_fuse
        }
        let closest_p = Grid::closest_point_of_line_to_circle(start, d_pos, player.obj.pos);
        let r_player = player.obj.pos - closest_p;
        if r_player.norm() <= 16. {
            *vel -= 2. * vel.dot(&r_player)/r_player.norm_squared() * r_player;
            let clip = (start + d_pos) - closest_p;

            obj.pos = closest_p + clip -  2. * clip.dot(&r_player)/r_player.norm_squared()*r_player;
            return GrenadeUpdate::None;
        }
        for enem in enemies.iter_mut() {
            let closest_e = Grid::closest_point_of_line_to_circle(start, d_pos, enem.pl.obj.pos);
            let r_enemy = enem.pl.obj.pos - closest_e;
            if r_enemy.norm() <= 16. {
                *vel -= 2. * vel.dot(&r_enemy)/r_enemy.norm_squared() * r_enemy;
                let clip = (start + d_pos) - closest_p;

                obj.pos = closest_e + clip - 2. * clip.dot(&r_enemy)/r_enemy.norm_squared()*r_enemy;
                return GrenadeUpdate::None;
            }
        }
        let cast = grid.ray_cast(palette, start, d_pos, true);
        obj.pos = cast.into_point();
        if let Some(to_wall) = cast.half_vec() {
            let clip = cast.clip();
            obj.pos += clip -  2. * clip.dot(&to_wall)/to_wall.norm_squared() * to_wall;
            *vel -= 2. * vel.dot(&to_wall)/to_wall.norm_squared() * to_wall;
        }
        change_fuse
    }
    pub fn update_cooked(ctx: &mut Context, obj: &mut Object, fuse: &mut f32, palette: &Palette, grid: &Grid, player: &mut Player, enemies: &mut [Enemy]) -> GrenadeUpdate {
        obj.pos = player.obj.pos + 20. * angle_to_vec(player.obj.rot);
        let change_fuse = Self::fuse_update(obj, fuse, palette, grid, player, enemies);
        if change_fuse == GrenadeUpdate::None  && !mouse::button_pressed(ctx, MouseButton::Right) {
            return GrenadeUpdate::Thrown{fuse: *fuse};
        }
        change_fuse
    }
    pub fn update(&mut self, ctx: &mut Context, a: &Assets, palette: &Palette, grid: &Grid, player: &mut Player, enemies: &mut [Enemy]) -> GameResult<GrenadeUpdate> {
        let update = match self.state {
            GrenadeState::Explosion{ref mut alive_time, ..} => {
                *alive_time += DELTA;
                if *alive_time >= EXPLOSION_LIFETIME {
                    GrenadeUpdate::Dead
                } else {
                    GrenadeUpdate::None
                }
            }
            GrenadeState::Cooked{ref mut fuse} => {
                Self::update_cooked(ctx, &mut self.obj, fuse, palette, grid, player, enemies)
            }
            GrenadeState::Fused{ref mut fuse} => {
                Self::update_fused(&mut self.obj, &mut self.vel, fuse, palette, grid, player, enemies)
            }
        };
        if let GrenadeUpdate::Thrown{ref fuse} = update {
            self.state = GrenadeState::Fused{
                fuse: *fuse,
            };
        }
        if let GrenadeUpdate::Explosion{..} = update {
            self.state = GrenadeState::Explosion {
                alive_time: 0.,
                mesh: self.make_mesh(ctx, a, palette, grid)?
            };
        }
        Ok(update)
    }
}

impl Utilities {
    pub fn cook_grenade(&mut self, ctx: &mut Context, mplayer: &mut MediaPlayer) -> GameResult<Option<GrenadeMaker>> {
        if self.grenades > 0 {
            self.grenades -= 1;

            Ok(Some(GrenadeMaker(620.)))
        } else {
            mplayer.play(ctx, "cock")?;
            Ok(None)
        }
    }
}

pub struct GrenadeMaker(f32);
impl GrenadeMaker {
    pub fn make(self, mut obj: Object) -> Grenade {
        let vel = angle_to_vec(obj.rot) * self.0;
        obj.rot = 0.;
        Grenade {
            state: GrenadeState::Cooked{fuse: 1.5},
            vel,
            obj,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GrenadeUpdate {
    Thrown {
        fuse: f32,
    },
    Explosion {
        player_hit: bool,
        enemy_hits: Vec<usize>,
    },
    Dead,
    None,
}
