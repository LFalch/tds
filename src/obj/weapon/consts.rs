use super::{FireMode, ReloadMode, Weapon, WeaponSlot};
use crate::util::{sstr, add_sstr, Sstr};

use lazy_static::lazy_static;

use std::fs::File;
use std::io::Read;
use std::num::NonZeroU16;
use std::collections::HashMap;
use std::f32::consts::PI;

lazy_static!{
    pub static ref WEAPONS: HashMap<&'static str, Weapon> = {
        let mut file = File::open("resources/weapons/specs.toml").expect("specs.toml file");
        let mut file_contents = String::new();
        file.read_to_string(&mut file_contents).expect("Reading to succeed");

        let templates: HashMap<Box<str>, WeaponTemplate> = toml::from_str(&file_contents).expect("well-defined weapons");
        templates.into_iter().map(|(k, v)| {
            let k = sstr(k);
            (k, v.build(k))
        }).collect()
    };
}

#[derive(Serialize, Deserialize)]
pub struct WeaponTemplate {
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    name: Sstr,
    clip_size: NonZeroU16,
    clips: NonZeroU16,
    damage: f32,
    penetration: f32,
    fire_rate: f32,
    reload_time: f32,
    #[serde(default)]
    reload_mode: ReloadMode,
    fire_mode: FireMode,
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    shot_snd: Sstr,
    #[serde(default = "def_cock")]
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    cock_snd: Sstr,
    #[serde(default = "def_reload")]
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    reload_snd: Sstr,
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    click_snd: Sstr,
    #[serde(default = "def_impact")]
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    impact_snd: Sstr,
    #[serde(deserialize_with = "crate::util::deserialize_sstr")]
    entity_sprite: Sstr,
    spray_pattern: Vec<f32>,
    spray_decay: f32,
    spray_repeat: usize,
    #[serde(default = "def_speed")]
    bullet_speed: f32,
    slot: WeaponSlot,
}

#[inline]
const fn def_speed() -> f32 {
    1200.
}
fn def_cock() -> Sstr {
    add_sstr("cock")
}
#[inline]
fn def_reload() -> Sstr {
    add_sstr("reload")
}
#[inline]
fn def_impact() -> Sstr {
    add_sstr("impact")
}

const DEG2RAD: f32 = PI / 180.;

impl WeaponTemplate {
    fn build(self, id: &'static str) -> Weapon {
        let WeaponTemplate {
            name,
            clip_size,
            clips,
            damage,
            penetration,
            fire_rate,
            reload_time,
            reload_mode,
            fire_mode,
            shot_snd,
            cock_snd,
            reload_snd,
            click_snd,
            impact_snd,
            entity_sprite,
            spray_pattern,
            spray_decay,
            spray_repeat,
            bullet_speed,
            slot
        } = self;

        Weapon {
            id,
            name,
            clip_size,
            clips,
            damage,
            penetration,
            fire_rate,
            reload_time,
            reload_mode,
            fire_mode,
            shot_snd,
            cock_snd,
            reload_snd,
            click_snd,
            impact_snd,
            hands_sprite: sstr(entity_sprite.to_string() + "_hands"),
            entity_sprite,
            spray_pattern: spray_pattern.into_iter().map(|deg| deg * DEG2RAD).collect(),
            spray_decay,
            spray_repeat,
            bullet_speed,
            slot
        }
    }
}
