use crate::{
    util::{TRANS, BLUE, GREEN,
    Vector2, Point2},
    io::tex::{Sprite, PosText},
    io::snd::Sound,
    ext::BoolExt,
    obj::{Object, enemy::Enemy, decoration::{DecorationObj, DECORATIONS}, pickup::PICKUPS, weapon::WEAPONS}
};
use ggez::{
    Context, GameResult,
    graphics::{self, Color, Rect, DrawMode},
    error::GameError,
    event::{MouseButton, KeyCode}
};

use super::{
    DELTA, Content, GameState, State, StateSwitch,
    world::{Grid, Level, Material}
};

use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone)]
enum Tool {
    Inserter(Insertion),
    Selector(Selection),
}

#[derive(Debug, Clone, Copy)]
enum Insertion {
    Material(Material),
    Intel,
    Enemy{rot: f32},
    Pickup(u8),
    Weapon(u8),
    Decoration{i: usize, rot: f32},
    Exit,
}
impl ::std::cmp::PartialEq for Insertion {
    fn eq(&self, rhs: &Self) -> bool {
        use self::Insertion::*;
        match (self, rhs) {
            (Material(m), Material(n)) if m == n => true,
            (Intel, Intel) => true,
            (Enemy{..}, Enemy{..}) => true,
            (Pickup(i), Pickup(j)) if i == j => true,
            (Weapon(i), Weapon(j)) if i == j => true,
            (Decoration{i, ..}, Decoration{i: j, ..}) if i == j => true,
            (Exit, Exit) => true,
            _ => false
        }
    }
}

#[derive(Default, Debug, PartialEq, Clone)]
struct Selection {
    exit: bool,
    enemies: Vec<usize>,
    intels: Vec<usize>,
    pickups: Vec<usize>,
    weapons: Vec<usize>,
    decorations: Vec<usize>,
    moving: Option<Point2>,
}

/// The state of the game
pub struct Editor {
    save: PathBuf,
    pos: Point2,
    level: Level,
    current: Tool,
    mat_text: PosText,
    entities_bar: InsertionBar,
    extra_bar: InsertionBar,
    draw_visibility_cones: bool,
    rotation_speed: f32,
    snap_on_grid: bool,
}

const PALETTE: [Material; 9] = [
    Material::Grass,
    Material::Dirt,
    Material::Floor,
    Material::Wall,
    Material::Asphalt,
    Material::Sand,
    Material::Concrete,
    Material::WoodFloor,
    Material::Stairs,
];

struct InsertionBar {
    ent_text: PosText,
    palette: &'static [EntityItem]
}

type EntityItem = (Sprite, Insertion);

impl InsertionBar {
    #[allow(clippy::new_ret_no_self)]
    fn new(p: Point2, ctx: &mut Context, s: &State, text: &str, palette: &'static [EntityItem]) -> GameResult<Self> {
        let ent_text = s.assets.text(ctx, p, text)?;
        Ok(Self {
            ent_text,
            palette
        })
    }
    fn draw(&self, ctx: &mut Context, s: &State, cur: Option<Insertion>) -> GameResult<()> {
        let mut drawparams = graphics::DrawParam {
            dest: self.ent_text.pos + Vector2::new(98., 16.),
            offset: Point2::new(0.5, 0.5),
            color: Some(graphics::WHITE),
            .. Default::default()
        };

        for (spr, ins) in self.palette {
            if let Some(cur) = cur {
                if ins == &cur {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, drawparams.dest, 17., 0.5)?;
                }
            }
            graphics::draw_ex(ctx, s.assets.get_img(*spr), drawparams)?;
            drawparams.dest.x += 34.; 
        }
        Ok(())
    }
    fn click(&self, mouse: Point2) -> Option<Insertion> {
        if mouse.y >= self.ent_text.pos.y && mouse.y < self.ent_text.pos.y+32. {
            let mut range = self.ent_text.pos.x + 82.;
            for (_, ins) in self.palette {
                if mouse.x >= range && mouse.x < range + 32. {
                    return Some(*ins);
                }
                range += 34.;
            }
        }
        None
    }
}

impl Editor {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(ctx: &mut Context, s: &State, level: Option<Level>) -> GameResult<Box<dyn GameState>> {
        let mat_text = s.assets.text(ctx, Point2::new(2., 18.0), "Materials:")?;
        let entities_bar = InsertionBar::new(Point2::new(392., 18.0), ctx, s, "Entitites:", &[
            (Sprite::Enemy, Insertion::Enemy{rot: 0.}),
            (Sprite::Goal, Insertion::Exit),
            (Sprite::Intel, Insertion::Intel),
            (Sprite::HealthPack, Insertion::Pickup(0)),
            (Sprite::Armour, Insertion::Pickup(1)),
            (Sprite::Adrenaline, Insertion::Pickup(2)),
            (Sprite::Glock, Insertion::Weapon(0)),
            (Sprite::FiveSeven, Insertion::Weapon(1)),
            (Sprite::Magnum, Insertion::Weapon(2)),
            (Sprite::M4, Insertion::Weapon(3)),
            (Sprite::Ak47, Insertion::Weapon(4)),
            (Sprite::Arwp, Insertion::Weapon(5)),
            (Sprite::Chair1, Insertion::Decoration{i: 0, rot: 0.}),
            (Sprite::Chair2, Insertion::Decoration{i: 1, rot: 0.}),
            (Sprite::ChairBoss, Insertion::Decoration{i: 2, rot: 0.}),
            (Sprite::LampPost, Insertion::Decoration{i: 3, rot: 0.}),
            (Sprite::OfficePlant, Insertion::Decoration{i: 4, rot: 0.}),
            (Sprite::OfficePlant2, Insertion::Decoration{i: 5, rot: 0.}),
            (Sprite::OfficePlant3, Insertion::Decoration{i: 6, rot: 0.}),
            (Sprite::Trashcan, Insertion::Decoration{i: 7, rot: 0.}),
        ])?;
        let extra_bar = InsertionBar::new(Point2::new(392., 52.0), ctx, s, "", &[
            (Sprite::ManholeCover, Insertion::Decoration{i: 8, rot: 0.}),
            (Sprite::ManholeCover2, Insertion::Decoration{i: 9, rot: 0.}),
            (Sprite::DeskLamp, Insertion::Decoration{i: 10, rot: 0.}),
            (Sprite::WallLight, Insertion::Decoration{i: 11, rot: 0.}),
            (Sprite::WallLight2, Insertion::Decoration{i: 12, rot: 0.}),
            (Sprite::WallLight3, Insertion::Decoration{i: 13, rot: 0.}),
            (Sprite::RoadMark, Insertion::Decoration{i: 14, rot: 0.}),
        ])?;

        let save;
        if let Content::File(ref f) = s.content {
            save = f.clone();
        } else {
            return Err(GameError::UnknownError("Cannot load editor without file".to_owned()));
        }

        let level = level
            .or_else(|| Level::load(&save).ok())
            .unwrap_or_else(|| Level::new(32, 32));

        let x = f32::from(level.grid.width()) * 16.;
        let y = f32::from(level.grid.height()) * 16.;

        Ok(Box::new(Editor {
            save,
            pos: Point2::new(x, y),
            current: Tool::Selector(Selection::default()),
            draw_visibility_cones: false,
            mat_text,
            entities_bar,
            extra_bar,
            level,
            rotation_speed: 0.,
            snap_on_grid: false,
        }))
    }
    fn mousepos(&self, s: &State) -> Point2 {
        let mut mp = s.mouse - s.offset;
        if self.snap_on_grid {
            mp.x = (mp.x / 32.).floor() * 32. + 16.;
            mp.y = (mp.y / 32.).floor() * 32. + 16.;
        }
        mp
    }
}

const START_X: f32 = 103.;
const YELLOW: Color = Color{r: 1., g: 1., b: 0., a: 1.};

impl GameState for Editor {
    fn update(&mut self, s: &mut State, _ctx: &mut Context) -> GameResult<()> {
        let speed = if s.modifiers.shift { 315. } else { 175. };
        let v = speed * Vector2::new(s.input.hor(), s.input.ver());
        self.pos += v * DELTA;

        match self.current {
            Tool::Inserter(Insertion::Enemy{ref mut rot}) => *rot += self.rotation_speed * DELTA,
            Tool::Inserter(Insertion::Decoration{ref mut rot, ..}) => *rot += self.rotation_speed * DELTA,
            _ => (),
        }
        Ok(())
    }
    fn logic(&mut self, s: &mut State, _ctx: &mut Context) -> GameResult<()> {
        if s.mouse_down.left && s.mouse.y > 64. {
            if let Tool::Inserter(Insertion::Material(mat)) = self.current {
                let (mx, my) = Grid::snap(s.mouse - s.offset);
                self.level.grid.insert(mx, my, mat);
            }
        }

        s.focus_on(self.pos);
        Ok(())
    }

    #[allow(clippy::cognitive_complexity)]
    fn draw(&mut self, s: &State, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, graphics::WHITE)?;
        self.level.grid.draw(ctx, &s.assets)?;

        if let Tool::Inserter(Insertion::Material(mat)) = self.current {
            let (x, y) = Grid::snap(s.mouse-s.offset);
            let x = f32::from(x) * 32.;
            let y = f32::from(y) * 32.;
            graphics::set_color(ctx, TRANS)?;
            mat.draw(ctx, &s.assets, x, y)?;
        }

        if let Some(start) = self.level.start_point {
            graphics::set_color(ctx, GREEN)?;
            graphics::circle(ctx, DrawMode::Fill, start, 16., 1.)?;
            graphics::set_color(ctx, BLUE)?;
            graphics::circle(ctx, DrawMode::Fill, start, 9., 1.)?;
        }
        if let Some(exit) = self.level.exit {
            if let Tool::Selector(Selection{exit: true, ..}) = self.current {
                graphics::set_color(ctx, YELLOW)?;
                graphics::circle(ctx, DrawMode::Fill, exit, 17., 0.5)?;
            }
            let drawparams = graphics::DrawParam {
                dest: exit,
                offset: Point2::new(0.5, 0.5),
                color: Some(graphics::WHITE),
                .. Default::default()
            };
            graphics::draw_ex(ctx, s.assets.get_img(Sprite::Goal), drawparams)?;
        }

        for (i, &intel) in self.level.intels.iter().enumerate() {
            if let Tool::Selector(Selection{ref intels, ..}) = self.current {
                if intels.contains(&i) {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, intel, 17., 0.5)?;
                }
            }
            let drawparams = graphics::DrawParam {
                dest: intel,
                offset: Point2::new(0.5, 0.5),
                color: Some(graphics::WHITE),
                .. Default::default()
            };
            graphics::draw_ex(ctx, s.assets.get_img(Sprite::Intel), drawparams)?;
        }

        graphics::set_color(ctx, graphics::WHITE)?;
        for (i, enemy) in self.level.enemies.iter().enumerate() {
            if let Tool::Selector(Selection{ref enemies, ..})= self.current {
                if enemies.contains(&i) {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, enemy.pl.obj.pos, 17., 0.5)?;
                }
            }
            if self.draw_visibility_cones {
                graphics::set_color(ctx, BLUE)?;
                enemy.draw_visibility_cone(ctx, 512.)?;
            }
            graphics::set_color(ctx, graphics::WHITE)?;
            enemy.draw(ctx, &s.assets)?;
        }
        for (i, decoration) in self.level.decorations.iter().enumerate() {
            if let Tool::Selector(Selection{ref decorations, ..})= self.current {
                if decorations.contains(&i) {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, decoration.obj.pos, 17., 0.5)?;
                }
            }
            graphics::set_color(ctx, graphics::WHITE)?;
            decoration.draw(ctx, &s.assets)?;
        }

        // Draw init pick-up-ables on top of enemies so they're visible
        for (i, pickup) in self.level.pickups.iter().enumerate() {
            if let Tool::Selector(Selection{ref pickups, ..}) = self.current {
                if pickups.contains(&i) {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, pickup.0, 17., 0.5)?;
                }
            }
            PICKUPS[pickup.1 as usize].draw(pickup.0, ctx, &s.assets)?;
        }
        for (i, weapon) in self.level.weapons.iter().enumerate() {
            if let Tool::Selector(Selection{ref weapons, ..}) = self.current {
                if weapons.contains(&i) {
                    graphics::set_color(ctx, YELLOW)?;
                    graphics::circle(ctx, DrawMode::Fill, weapon.pos, 17., 0.5)?;
                }
            }
            let drawparams = graphics::DrawParam {
                dest: weapon.pos,
                offset: Point2::new(0.5, 0.5),
                color: Some(graphics::WHITE),
                .. Default::default()
            };
            graphics::draw_ex(ctx, s.assets.get_img(weapon.weapon.entity_sprite), drawparams)?;
        }

        // Draw moving objects shadows
        if let Tool::Selector(ref selection @ Selection{moving: Some(_), ..}) = self.current {
            let mousepos = self.mousepos(s);
            let dist = mousepos - selection.moving.unwrap();

            graphics::set_color(ctx, TRANS)?;
            for &i in &selection.enemies {
                let mut enem = self.level.enemies[i].clone();
                enem.pl.obj.pos += dist;
                enem.draw(ctx, &s.assets)?;
            }
            for &i in &selection.intels {
                let drawparams = graphics::DrawParam {
                    dest: self.level.intels[i] + dist,
                    offset: Point2::new(0.5, 0.5),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(Sprite::Intel), drawparams)?;
            }
            for &i in &selection.decorations {
                let mut dec = self.level.decorations[i].clone();
                dec.obj.pos += dist;
                dec.draw(ctx, &s.assets)?;
            }
            for &i in &selection.pickups {
                let pickup = self.level.pickups[i];
                let drawparams = graphics::DrawParam {
                    dest: pickup.0 + dist,
                    offset: Point2::new(0.5, 0.5),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(PICKUPS[pickup.1 as usize].spr), drawparams)?;
            }
            for &i in &selection.weapons {
                let drawparams = graphics::DrawParam {
                    dest: self.level.weapons[i].pos + dist,
                    offset: Point2::new(0.5, 0.5),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(self.level.weapons[i].weapon.entity_sprite), drawparams)?;
            }
            if selection.exit {
                if let Some(exit) = self.level.exit {
                    let drawparams = graphics::DrawParam {
                        dest: exit + dist,
                        offset: Point2::new(0.5, 0.5),
                        .. Default::default()
                    };
                    graphics::draw_ex(ctx, s.assets.get_img(Sprite::Goal), drawparams)?;
                }
            }
        }

        Ok(())
    }
    fn draw_hud(&mut self, s: &State, ctx: &mut Context) -> GameResult<()> {
        let dest = self.mousepos(s) + s.offset;
        match self.current {
            Tool::Selector(_) => (),
            Tool::Inserter(Insertion::Material(_)) => (),
            Tool::Inserter(Insertion::Pickup(index)) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    rotation: 0.,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(PICKUPS[index as usize].spr), drawparams)?;
            }
            Tool::Inserter(Insertion::Weapon(index)) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    rotation: 0.,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(WEAPONS[index as usize].entity_sprite), drawparams)?;
            }
            Tool::Inserter(Insertion::Enemy{rot}) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    rotation: rot,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(Sprite::Enemy), drawparams)?;
            }
            Tool::Inserter(Insertion::Decoration{i, rot}) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    rotation: rot,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(DECORATIONS[i].spr), drawparams)?;
            }
            Tool::Inserter(Insertion::Exit) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(Sprite::Goal), drawparams)?;
            }
            Tool::Inserter(Insertion::Intel) => {
                let drawparams = graphics::DrawParam {
                    dest,
                    offset: Point2::new(0.5, 0.5),
                    color: Some(TRANS),
                    .. Default::default()
                };
                graphics::draw_ex(ctx, s.assets.get_img(Sprite::Intel), drawparams)?;
            }
        }

        graphics::set_color(ctx, Color{r: 0.5, g: 0.5, b: 0.5, a: 1.})?;
        graphics::rectangle(ctx, DrawMode::Fill, Rect{x:0.,y:0.,h: 64., w: s.width as f32})?;
        graphics::set_color(ctx, graphics::WHITE)?;

        for (i, mat) in PALETTE.iter().enumerate() {
            let x = START_X + i as f32 * 36.;
            if Tool::Inserter(Insertion::Material(*mat)) == self.current {
                graphics::set_color(ctx, YELLOW)?;
                graphics::rectangle(ctx, DrawMode::Fill, Rect{x: x - 1., y: 15., w: 34., h: 34.})?;
                graphics::set_color(ctx, graphics::WHITE)?;
            }
            mat.draw(ctx, &s.assets, x, 16.)?;
        }

        self.entities_bar.draw(ctx, s, if let Tool::Inserter(ins) = self.current{Some(ins)}else{None})?;
        self.extra_bar.draw(ctx, s, if let Tool::Inserter(ins) = self.current{Some(ins)}else{None})?;

        graphics::set_color(ctx, graphics::WHITE)?;
        self.mat_text.draw_text(ctx)?;
        self.entities_bar.ent_text.draw_text(ctx)?;
        self.extra_bar.ent_text.draw_text(ctx)
    }
    fn key_up(&mut self, s: &mut State, _ctx: &mut Context, keycode: Keycode) {
        use self::Keycode::*;
        match keycode {
            Z => self.level.save(&self.save).unwrap(),
            X => self.level = Level::load(&self.save).unwrap(),
            C => self.draw_visibility_cones.toggle(),
            G => self.snap_on_grid.toggle(),
            P => {
                s.switch(StateSwitch::Play(self.level.clone()));
            }
            T => self.current = Tool::Selector(Selection::default()),
            Delete | Backspace => if let Tool::Selector(ref mut selection) = self.current {
                #[allow(clippy::unneeded_field_pattern)]
                let Selection {
                    mut enemies,
                    mut intels,
                    mut pickups,
                    mut weapons,
                    mut decorations,
                    exit, moving: _,
                } = ::std::mem::replace(selection, Selection::default());

                if exit {
                    self.level.exit = None;
                }
                enemies.sort();
                for enemy in enemies.into_iter().rev() {
                    self.level.enemies.remove(enemy);
                }
                intels.sort();
                for intel in intels.into_iter().rev() {
                    self.level.intels.remove(intel);
                }
                decorations.sort();
                for decoration in decorations.into_iter().rev() {
                    self.level.decorations.remove(decoration);
                }
                pickups.sort();
                for pickup in pickups.into_iter().rev() {
                    self.level.pickups.remove(pickup);
                }
                weapons.sort();
                for weapon in weapons.into_iter().rev() {
                    self.level.weapons.remove(weapon);
                }
            }
            Comma => {
                self.rotation_speed = 0.;
                if s.modifiers.shift {
                    match self.current {
                        Tool::Inserter(Insertion::Enemy{ref mut rot}) => *rot -= std::f32::consts::FRAC_PI_4,
                        Tool::Inserter(Insertion::Decoration{ref mut rot, ..}) => *rot -= std::f32::consts::FRAC_PI_4,
                        _ => (),
                    }
                }
            }
            Period => {
                self.rotation_speed = 0.;
                if s.modifiers.shift {
                    match self.current {
                        Tool::Inserter(Insertion::Enemy{ref mut rot}) => *rot += std::f32::consts::FRAC_PI_4,
                        Tool::Inserter(Insertion::Decoration{ref mut rot, ..}) => *rot += std::f32::consts::FRAC_PI_4,
                        _ => (),
                    }
                }
            }
            Up if s.modifiers.ctrl => self.level.grid.shorten(),
            Down if s.modifiers.ctrl => self.level.grid.heighten(),
            Left if s.modifiers.ctrl => self.level.grid.thin(),
            Right if s.modifiers.ctrl => self.level.grid.widen(),
            _ => return,
        }
    }
    fn mouse_down(&mut self, s: &mut State, _ctx: &mut Context, btn: MouseButton) {
        use self::MouseButton::*;
        let mousepos = self.mousepos(&s);
        if let Left = btn {
            if let Tool::Selector(ref mut selection) = self.current {
                for &i in &selection.enemies {
                    if (self.level.enemies[i].pl.obj.pos - mousepos).norm() <= 16. {
                        return selection.moving = Some(mousepos);
                    }
                }
                for &i in &selection.intels {
                    if (self.level.intels[i] - mousepos).norm() <= 16. {
                        return selection.moving = Some(mousepos);
                    }
                }
                for &i in &selection.decorations {
                    if (self.level.decorations[i].obj.pos - mousepos).norm() <= 16. {
                        return selection.moving = Some(mousepos);
                    }
                }
                for &i in &selection.pickups {
                    if (self.level.pickups[i].0 - mousepos).norm() <= 16. {
                        return selection.moving = Some(mousepos);
                    }
                }
                for &i in &selection.weapons {
                    if (self.level.weapons[i].pos - mousepos).norm() <= 16. {
                        return selection.moving = Some(mousepos);
                    }
                }
                if selection.exit {
                    if let Some(exit) = self.level.exit {
                        if (exit - mousepos).norm() <= 16. {
                            return selection.moving = Some(mousepos);
                        }
                    }
                }
            }
        }
    }
    fn mouse_up(&mut self, s: &mut State, ctx: &mut Context, btn: MouseButton) {
        use self::MouseButton::*;
        let mousepos = self.mousepos(&s);
        match btn {
            Left => {
                
            if let Some(ins) = self.extra_bar.click(s.mouse) {
                self.current = Tool::Inserter(ins);
                return
            }
                
            if s.mouse.y <= 64. {
                if s.mouse.x > START_X && s.mouse.x < START_X + PALETTE.len() as f32 * 36. {
                    let i = ((s.mouse.x - START_X) / 36.) as usize;

                    self.current = Tool::Inserter(Insertion::Material(PALETTE[i]));
                }
                if let Some(ins) = self.entities_bar.click(s.mouse) {
                    self.current = Tool::Inserter(ins);
                }
            } else {
                match self.current {
                    Tool::Inserter(Insertion::Material(_)) => (),
                    Tool::Selector(ref mut selection) => {

                        if let Some(moved_from) = selection.moving {
                            let dist = mousepos - moved_from;

                            if selection.exit {
                                if let Some(ref mut exit) = self.level.exit {
                                    *exit += dist;
                                }
                            }
                            for i in selection.enemies.iter().rev() {
                                self.level.enemies[*i].pl.obj.pos += dist;
                            }
                            for i in selection.intels.iter().rev() {
                                self.level.intels[*i] += dist;
                            }
                            for i in selection.decorations.iter().rev() {
                                self.level.decorations[*i].obj.pos += dist;
                            }
                            for i in selection.pickups.iter().rev() {
                                self.level.pickups[*i].0 += dist;
                            }
                            for i in selection.weapons.iter().rev() {
                                self.level.weapons[*i].pos += dist;
                            }
                            selection.moving = None;
                        } else {
                            if !s.modifiers.ctrl {
                                *selection = Selection::default();
                            }
                            for (i, enemy) in self.level.enemies.iter().enumerate() {
                                if (enemy.pl.obj.pos - mousepos).norm() <= 16. && !selection.enemies.contains(&i) {
                                    selection.enemies.push(i);
                                    return
                                }
                            }
                            if let Some(exit) = self.level.exit {
                                if (exit - mousepos).norm() <= 16. && !selection.exit {
                                    selection.exit = true;
                                    return
                                }
                            }
                            for (i, &intel) in self.level.intels.iter().enumerate() {
                                if (intel - mousepos).norm() <= 16. && !selection.intels.contains(&i) {
                                    selection.intels.push(i);
                                    return
                                }
                            }
                            for (i, decoration) in self.level.decorations.iter().enumerate() {
                                if (decoration.obj.pos - mousepos).norm() <= 16. && !selection.decorations.contains(&i) {
                                    selection.decorations.push(i);
                                    return
                                }
                            }
                            for (i, &pickup) in self.level.pickups.iter().enumerate() {
                                if (pickup.0 - mousepos).norm() <= 16. && !selection.pickups.contains(&i) {
                                    selection.pickups.push(i);
                                    return
                                }
                            }
                            for (i, weapon) in self.level.weapons.iter().enumerate() {
                                if (weapon.pos - mousepos).norm() <= 16. && !selection.weapons.contains(&i) {
                                    selection.weapons.push(i);
                                    return
                                }
                            }
                        }
                    }
                    Tool::Inserter(Insertion::Exit) => {
                        self.level.exit = Some(self.mousepos(&s));
                        self.current = Tool::Selector(Selection{exit: true, .. Default::default()});
                    }
                    Tool::Inserter(Insertion::Enemy{rot}) => {
                        s.mplayer.play(ctx, Sound::Reload).unwrap();
                        self.level.enemies.push(Enemy::new(Object::with_rot(mousepos, rot)));
                        self.level.weapons.push(WEAPONS[0].make_drop(mousepos));
                    },
                    Tool::Inserter(Insertion::Decoration{i, rot}) => {
                        self.level.decorations.push(DecorationObj::new(Object::with_rot(mousepos, rot), i));
                    },
                    Tool::Inserter(Insertion::Pickup(i)) => {
                        self.level.pickups.push((mousepos, i));
                    },
                    Tool::Inserter(Insertion::Weapon(i)) => {
                        self.level.weapons.push(WEAPONS[i as usize].make_drop(mousepos));
                    },
                    Tool::Inserter(Insertion::Intel) => self.level.intels.push(mousepos),
                }
            }}
            Middle => self.level.start_point = Some(self.mousepos(&s)),
            _ => ()
        }
    }
    fn key_down(&mut self, s: &mut State, _ctx: &mut Context, keycode: Keycode) {
        use self::Keycode::*;
        match keycode {
            Comma if !s.modifiers.shift => self.rotation_speed -= 6.,
            Period if !s.modifiers.shift => self.rotation_speed += 6.,
            Q => self.level.start_point = Some(self.mousepos(&s)),
            _ => return,
        }
    }
}
