use std::num::NonZeroU16;
use std::fmt::{self, Display};

use crate::{
    util::{Point2, angle_to_vec, Sstr},
    game::DELTA,
    io::{
        snd::MediaPlayer,
        tex::{PosText, Assets},
    },
};
use ggez::{Context, GameResult};

use super::{Object, bullet::Bullet};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FireMode {
    Automatic,
    SemiAutomatic,
    BoltAction,
    PumpAction {
        shell_load: u16,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum WeaponSlot {
    Holster,
    Sling,
}

impl FireMode {
    #[inline]
    pub fn is_auto(self) -> bool {
        if let FireMode::Automatic = self {
            true
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReloadMode {
    Magazine,
    RoundByRound,
}

impl Default for ReloadMode {
    fn default() -> Self{
        ReloadMode::Magazine
    }
}

#[derive(Debug, Clone)]
pub struct Weapon {
    pub id: Sstr,
    pub name: Sstr,
    pub clip_size: NonZeroU16,
    pub clips: NonZeroU16,
    pub damage: f32,
    /// Fraction of armour damage rediverted to hp damage
    pub penetration: f32,
    /// Time between each shot
    pub fire_rate: f32,
    /// Time to reload a new clip/magazine
    pub reload_time: f32,
    pub reload_mode: ReloadMode,
    pub fire_mode: FireMode,
    pub shot_snd: Sstr,
    pub cock_snd: Sstr,
    pub reload_snd: Sstr,
    pub click_snd: Sstr,
    pub impact_snd: Sstr,
    pub entity_sprite: Sstr,
    pub hands_sprite: Sstr,
    pub spray_pattern: Box<[f32]>,
    pub spray_decay: f32,
    pub spray_repeat: usize,
    pub bullet_speed: f32,
    pub slot: WeaponSlot,
}

mod consts;
pub use self::consts::*;

impl Weapon {
    pub fn make_instance(&self) -> WeaponInstance<'_> {
        let cur_clip = self.clip_size.get();
        WeaponInstance {
            weapon: self,
            cur_clip,
            loading_time: 0.,
            jerk: 0.,
            jerk_decay: 0.,
            spray_index: 0,
            ammo: cur_clip*self.clips.get(),
            is_reloading: false,
        }
    }
    pub fn make_drop(&self, pos: Point2) -> WeaponDrop<'_> {
        let cur_clip = self.clip_size.get();
        WeaponDrop {
            pos,
            cur_clip,
            ammo: cur_clip*self.clips.get(),
            weapon: self,
        }
    }
    #[inline]
    pub fn get_bullet_spr(&self) -> &str {
        "common/bullet"
    }
}

#[derive(Debug, Clone)]
pub struct WeaponDrop<'a> {
    pub pos: Point2,
    pub cur_clip: u16,
    pub ammo: u16,
    pub weapon: &'a Weapon,
}

impl Display for WeaponDrop<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}/{}", self.weapon.name, self.cur_clip, self.ammo)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct WeaponInstance<'a> {
    pub cur_clip: u16,
    pub ammo: u16,
    pub loading_time: f32,
    pub jerk: f32,
    pub jerk_decay: f32,
    pub spray_index: usize,
    pub weapon: &'a Weapon,
    pub is_reloading: bool,
}

impl Display for WeaponInstance<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}/{}", self.weapon.name, self.cur_clip, self.ammo)
    }
}

impl<'a> WeaponInstance<'a> {
    pub fn weapon_text(p: Point2, a: &Assets) -> PosText {
        a.text(p).and_text("BFG").and_text(" ").and_text("0").and_text("/").and_text("0").and_text(" (").and_text("0").and_text(" ").and_text("0").and_text("s)")
    }
    pub fn update_text(&self, text: &mut PosText) -> GameResult<()> {
        text
            .update(0, &*self.weapon.name)?
            .update(2, format!("{}", self.cur_clip))?
            .update(4, format!("{}", self.ammo))?
            .update(6, format!("{:.3}", self.jerk))?
            .update(8, format!("{:.1}", self.jerk_decay))?;
        Ok(())
    } 
    pub fn into_drop(self, pos: Point2) -> WeaponDrop<'a> {
        let WeaponInstance{cur_clip, ammo, weapon, ..} = self;
        WeaponDrop {
            pos,
            cur_clip,
            ammo,
            weapon,
        }
    }
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_drop(wd: WeaponDrop<'a>) -> Self {
        let WeaponDrop{cur_clip, ammo, weapon, ..} = wd;
        Self {
            loading_time: 0.,
            jerk: 0.,
            jerk_decay: 0.,
            spray_index: 0,
            cur_clip,
            ammo,
            weapon,
            is_reloading: false,
        }
    }
    pub fn update(&mut self, ctx: &mut Context, mplayer: &mut MediaPlayer) -> GameResult<()> {
        if self.jerk_decay <= DELTA {
            self.jerk = 0.;
            self.jerk_decay = 0.;
            self.spray_index = 0;
        } else {
            self.jerk_decay -= DELTA;
        }
        if self.is_reloading {
            self.reload(ctx, mplayer)?;
        }
        if self.loading_time <= DELTA {
            self.loading_time = 0.;
        } else {
            self.loading_time -= DELTA;
            if self.loading_time <= DELTA && ReloadMode::RoundByRound != self.weapon.reload_mode {
                mplayer.play(ctx, &self.weapon.cock_snd)?;
            }
        }
        Ok(())
    }
    pub fn reload(&mut self, ctx: &mut Context, mplayer: &mut MediaPlayer) -> GameResult<()> {
        let clip_size = self.weapon.clip_size.get();
        if self.cur_clip == clip_size || self.ammo == 0 {
            self.is_reloading = false;
            return Ok(())
        }
        if self.loading_time != 0. {
            return Ok(())
        }
        self.loading_time = self.weapon.reload_time;
        mplayer.play(ctx, &self.weapon.reload_snd)?;
        if !self.is_reloading {
            self.is_reloading = true;
            return Ok(())
        }
        let ammo_to_reload: u16;
        if let ReloadMode::RoundByRound = self.weapon.reload_mode { 
            ammo_to_reload = 1;
        } else {
            ammo_to_reload = clip_size - self.cur_clip;
        }

        if self.ammo < ammo_to_reload {
            self.cur_clip += self.ammo;
            self.ammo = 0;
            self.is_reloading = false;
        } else {
            self.ammo -= ammo_to_reload;
            self.cur_clip += ammo_to_reload;
        }
        if self.cur_clip == clip_size {
            self.is_reloading = false;
            self.loading_time = 0.;
        }
        Ok(())
    }
    fn next_jerk(&mut self) -> f32 {
        let jerk = self.jerk;

        self.jerk_decay = self.weapon.spray_decay;
        self.jerk += self.weapon.spray_pattern[self.spray_index];
        self.spray_index += 1; 
        if self.spray_index >= self.weapon.spray_pattern.len() {
            self.spray_index -= self.weapon.spray_repeat;
        }

        jerk
    }
    pub fn shoot(&mut self, ctx: &mut Context, mplayer: &mut MediaPlayer) -> GameResult<Option<BulletMaker<'a>>> {
        if ReloadMode::RoundByRound == self.weapon.reload_mode && self.is_reloading && self.cur_clip > 0 {
            self.is_reloading = false;
            self.loading_time = 0.;
        }
        if self.cur_clip > 0 && self.loading_time == 0. {
            self.cur_clip -= 1;
            if self.cur_clip > 0 {
                self.loading_time = self.weapon.fire_rate;
            }
            self.is_reloading = false;
            mplayer.play(ctx, &self.weapon.shot_snd)?;

            let jerks = match self.weapon.fire_mode {
                FireMode::PumpAction{shell_load} => {
                    (0..shell_load).map(|_| self.next_jerk()).collect()
                },
                _ => {
                    vec![self.next_jerk()]
                },
            };

            Ok(Some(BulletMaker(self.weapon, jerks)))
        } else {
            if self.cur_clip == 0 && !self.is_reloading {
                mplayer.play(ctx, &self.weapon.click_snd)?;
            }
            Ok(None)
        }
    }
}

// Weapon, jerks
// jerk is used to adjust the target position
pub struct BulletMaker<'a>(&'a Weapon, Vec<f32>);
impl<'a> BulletMaker<'a> {
    pub fn make(self, obj: Object) -> impl Iterator<Item=Bullet<'a>> {
        let BulletMaker(weapon, jerks) = self;

        jerks.into_iter().map(move |jerk| {
            let mut obj = obj.clone();

            obj.rot += jerk;
            Bullet {
                vel: weapon.bullet_speed * angle_to_vec(obj.rot),
                obj,
                weapon,
            }
        })
    }
}

