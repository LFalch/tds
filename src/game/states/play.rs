use std::f32::consts::FRAC_1_SQRT_2 as COS_45_D;
use crate::{
    ext::FloatExt,
    util::{
        BLUE, GREEN, RED,
        angle_to_vec, angle_from_vec,
        ver, hor,
        Vector2, Point2
    },
    io::tex::PosText,
    obj::{Object, bullet::Bullet, decal::Decal, pickup::Pickup, player::{Player, WepSlots, ActiveSlot}, enemy::{Enemy, Chaser}, health::Health, weapon::{self, WeaponInstance}, grenade::GrenadeUpdate},
    game::{
        DELTA, State, GameState, StateSwitch, world::{Level, Statistics, World},
        event::{Event::{self, Key, Mouse}, MouseButton, KeyCode, KeyMods}
    },
};
use ggez::{
    Context, GameResult,
    graphics::{
        self, Drawable, DrawMode, Rect,
        Color, DrawParam,
        MeshBuilder, Mesh, WHITE,
        spritebatch::SpriteBatch,
    },
    input::{
        keyboard,
        mouse,
    },
};

use rand::{thread_rng, prelude::SliceRandom};

pub fn new_blood(mut obj: Object) -> Decal {
    obj.pos += 16. * angle_to_vec(obj.rot);
    Decal {
        obj,
        spr: [
            "common/blood1",
            "common/blood2",
            "common/blood2",
            "common/blood3",
            "common/blood3",
        ].choose(&mut thread_rng()).copied().map(Into::into).unwrap(),
    }
}

/// The state of the game
pub struct Play {
    hp_text: PosText,
    arm_text: PosText,
    reload_text: PosText,
    wep_text: PosText,
    status_text: PosText,
    hud: Hud,
    world: World,
    holes: SpriteBatch,
    cur_pickup: Option<usize>,
    victory_time: f32,
    time: usize,
    initial: (Health, WepSlots),
    level: Level,
}

impl Play {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(ctx: &mut Context, s: &mut State, level: Level, pl: Option<(Health, WepSlots)>) -> GameResult<Box<dyn GameState>> {
        mouse::set_cursor_hidden(ctx, true);

        let mut player = Player::from_point(level.start_point.unwrap_or_else(|| Point2::new(500., 500.)));
        if let Some((h, w)) = pl {
            player = player.with_health(h).with_weapon(w);
        };

        Ok(Box::new(
            Play {
                level: level.clone(),
                initial: (player.health, player.wep.clone()),
                hp_text: s.assets.text(Point2::new(4., 4.)).and_text("100"),
                arm_text: s.assets.text(Point2::new(4., 33.)).and_text("100"),
                reload_text: s.assets.text(Point2::new(4., 62.)).and_text("0.0").and_text("s"),
                wep_text: WeaponInstance::weapon_text(Point2::new(2., 87.), &s.assets),
                status_text: s.assets.text(Point2::new(s.width as f32 / 2., s.height as f32 / 2. + 32.)).and_text(""),
                hud: Hud::new(ctx)?,
                time: 0,
                victory_time: 0.,
                cur_pickup: None,
                world: {
                    let mut world = World {
                        enemies: level.enemies,
                        bullets: Vec::new(),
                        grenades: Vec::new(),
                        weapons: level.weapons,
                        player,
                        palette: level.palette,
                        grid: level.grid,
                        exit: level.exit,
                        intels: level.intels,
                        decals: level.decals,
                        pickups: level.pickups.into_iter().map(|(p, i)| Pickup::new(p, i)).collect(),
                    };
                    world.enemy_pickup();
                    world.player_pickup();

                    if world.player.wep.get_active().is_none() {
                        warn!("player has no weapon");
                    }

                    for enemy_pos in world.enemies.iter().filter_map(|enemy| if enemy.pl.wep.get_active().is_none() {Some(enemy.pl.obj.pos)}else{None}) {
                        warn!("enemy at {:.2} has no weapon", enemy_pos)
                    }

                    world
                },
                holes: SpriteBatch::new(s.assets.get_img(ctx, "common/hole").clone()),
            }
        ))
    }
}

impl GameState for Play {
    #[allow(clippy::cognitive_complexity)]
    fn update(&mut self, s: &mut State, ctx: &mut Context) -> GameResult<()> {
        self.hp_text.update(0, format!("{:02.0}", self.world.player.health.hp))?;
        self.arm_text.update(0, format!("{:02.0}", self.world.player.health.armour))?;
        if let Some(wep) = self.world.player.wep.get_active() {
            self.reload_text.update(0, format!("{:.1}", wep.loading_time))?;
            wep.update_text(&mut self.wep_text)?;
        }
        if let Some(i) = self.cur_pickup {
            // TODO change text to say what's being swapped out
            self.status_text.text.fragments_mut()[0]= format!("Press F to pick up {}", self.world.weapons[i]).into();
        } else {
            self.status_text.update(0, "")?;
        }

        let mut deads = Vec::new();
        for (i, grenade) in self.world.grenades.iter_mut().enumerate().rev() {
            let g_update = grenade.update(ctx, &s.assets, &self.world.palette, &self.world.grid, &mut self.world.player, &mut *self.world.enemies)?;

            match g_update {
                GrenadeUpdate::Explosion{player_hit, enemy_hits} => {
                    s.mplayer.play(ctx, "boom")?;

                    if player_hit {
                        self.world.decals.push(new_blood(self.world.player.obj.clone()));
                        s.mplayer.play(ctx, "hit")?;

                        if self.world.player.health.is_dead() {
                            s.switch(StateSwitch::Lose(Box::new(Statistics{
                                time: self.time,
                                enemies_left: self.world.enemies.len(),
                                health_left: self.initial.0,
                                level: self.level.clone(),
                                weapon: self.initial.1.clone(),
                            })));
                            s.mplayer.play(ctx, "death")?;
                        } else {
                            s.mplayer.play(ctx, "hurt")?;
                        }
                    }
                    for i in enemy_hits {
                        let enemy = &self.world.enemies[i];
                        s.mplayer.play(ctx, "hit")?;

                        self.world.decals.push(new_blood(enemy.pl.obj.clone()));
                        if enemy.pl.health.is_dead() {
                            s.mplayer.play(ctx, "death")?;

                            let Enemy{pl: Player{wep, obj: Object{pos, ..}, ..}, ..}
                                = self.world.enemies.remove(i);
                            for wep in wep {
                                self.world.weapons.push(wep.into_drop(pos));
                            }
                        } else {
                            if !enemy.behaviour.chasing() {
                                self.world.enemies[i].behaviour = Chaser::LookAround{
                                    dir: grenade.obj.pos - enemy.pl.obj.pos
                                };
                            }
                            s.mplayer.play(ctx, "hurt")?;
                        }
                    }
                }
                GrenadeUpdate::Dead => {
                    deads.push(i);
                }
                GrenadeUpdate::Thrown{..} => {
                    s.mplayer.play(ctx, "throw")?;
                    grenade.obj.pos = self.world.player.obj.pos + 20. * angle_to_vec(self.world.player.obj.rot);
                    grenade.vel = grenade.vel.norm() * angle_to_vec(self.world.player.obj.rot);
                },
                GrenadeUpdate::None => (),
            }
        }
        for i in deads {
            self.world.grenades.remove(i);
        }

        let mut deads = Vec::new();
        for (i, bullet) in self.world.bullets.iter_mut().enumerate().rev() {
            let hit = bullet.update(&self.world.palette, &self.world.grid, &mut self.world.player, &mut *self.world.enemies);
            
            use crate::obj::bullet::Hit;

            match hit {
                Hit::None => (),
                Hit::Wall => {
                    s.mplayer.play(ctx, &bullet.weapon.impact_snd)?;
                    let dir = angle_to_vec(bullet.obj.rot);
                    bullet.obj.pos += Vector2::new(5.*dir.x.signum(), 5.*dir.y.signum());
                    self.holes.add(bullet.obj.drawparams());
                    deads.push(i);
                }
                Hit::Player => {
                    deads.push(i);
                    self.world.decals.push(new_blood(bullet.obj.clone()));
                    s.mplayer.play(ctx, "hit")?;

                    if self.world.player.health.is_dead() {
                        s.switch(StateSwitch::Lose(Box::new(Statistics{
                            time: self.time,
                            enemies_left: self.world.enemies.len(),
                            health_left: self.initial.0,
                            level: self.level.clone(),
                            weapon: self.initial.1.clone(),
                        })));
                        s.mplayer.play(ctx, "death")?;
                    } else {
                        s.mplayer.play(ctx, "hurt")?;
                    }
                }
                Hit::Enemy(e) => {
                    deads.push(i);
                    let enemy = &self.world.enemies[e];
                    s.mplayer.play(ctx, "hit")?;

                    self.world.decals.push(new_blood(bullet.obj.clone()));
                    if enemy.pl.health.is_dead() {
                        s.mplayer.play(ctx, "death")?;

                        let Enemy{pl: Player{wep, obj: Object{pos, ..}, ..}, ..}
                            = self.world.enemies.remove(e);
                        for wep in wep {
                            self.world.weapons.push(wep.into_drop(pos));
                        }
                    } else {
                        if !enemy.behaviour.chasing() {
                            self.world.enemies[e].behaviour = Chaser::LookAround{
                                dir: bullet.obj.pos - enemy.pl.obj.pos
                            };
                        }
                        s.mplayer.play(ctx, "hurt")?;
                    }
                }
            }
        }
        for i in deads {
            self.world.bullets.remove(i);
        }

        let mut deads = Vec::new();
        for (i, &intel) in self.world.intels.iter().enumerate().rev() {
            if (intel-self.world.player.obj.pos).norm() <= 15. {
                deads.push(i);
                s.mplayer.play(ctx, "hit")?;
            }
        }
        for i in deads {
            self.world.intels.remove(i);
        }
        let mut deads = Vec::new();
        for (i, pickup) in self.world.pickups.iter().enumerate().rev() {
            if (pickup.pos-self.world.player.obj.pos).norm() <= 15. && pickup.apply(&mut self.world.player.health) {
                deads.push(i);
                s.mplayer.play(ctx, "hit")?;
            }
        }
        for i in deads {
            self.world.pickups.remove(i);
        }
        self.cur_pickup = None;
        for (i, weapon) in self.world.weapons.iter().enumerate().rev() {
            if (weapon.pos-self.world.player.obj.pos).norm() <= 29. {
                self.cur_pickup = Some(i);
                break
            }
        }

        // Define player velocity here already because enemies need it
        let player_vel = Vector2::new(hor(&ctx), ver(&ctx));

        for enemy in self.world.enemies.iter_mut() {
            if enemy.can_see(self.world.player.obj.pos, &self.world.palette, &self.world.grid) {
                enemy.behaviour = Chaser::LastKnown{
                    pos: self.world.player.obj.pos,
                    vel: player_vel,
                };

                if let Some(wep) = enemy.pl.wep.get_active_mut() {
                    if let Some(bm) = wep.shoot(ctx, &mut s.mplayer)? {
                        let pos = enemy.pl.obj.pos + 20. * angle_to_vec(enemy.pl.obj.rot);
                        let mut bul = Object::new(pos);
                        bul.rot = enemy.pl.obj.rot;

                        for bullet in bm.make(bul) {
                            self.world.bullets.push(bullet);
                        }
                    }
                }
            }
            enemy.update(ctx, &mut s.mplayer)?;
        }

        let speed = if !keyboard::is_mod_active(ctx, KeyMods::SHIFT) {
            200.
        } else {
            100.
        };
        if let Some(wep) = self.world.player.wep.get_active_mut() {
            wep.update(ctx, &mut s.mplayer)?;
            if wep.cur_clip > 0 && mouse::button_pressed(ctx, MouseButton::Left) && wep.weapon.fire_mode.is_auto() {
                if let Some(bm) = wep.shoot(ctx, &mut s.mplayer)? {
                    let pos = self.world.player.obj.pos + 20. * angle_to_vec(self.world.player.obj.rot);
                    let mut bul = Object::new(pos);
                    bul.rot = self.world.player.obj.rot;

                    for bullet in bm.make(bul) {
                        self.world.bullets.push(bullet);
                    }
                }
            }
        }
        self.world.player.obj.move_on_grid(player_vel, speed, &self.world.palette, &self.world.grid);

        let game_won = match self.world.exit {
            Some(p) => self.world.intels.is_empty() && (p - self.world.player.obj.pos).norm() < 32.,
            None => self.world.enemies.is_empty(),
        };

        if game_won && self.victory_time <= 0. {
            s.mplayer.play(ctx, "victory")?;
            self.victory_time += DELTA;
        } else if self.victory_time > 0. {
            self.victory_time += DELTA;
        } else {
            self.time += 1;
        }
        if self.victory_time >= 2. {
            s.switch(StateSwitch::Win(Box::new(Statistics{
                level: self.level.clone(),
                time: self.time,
                enemies_left: self.world.enemies.len(),
                health_left: self.world.player.health,
                weapon: self.world.player.wep.clone(),
            })));
        }
        Ok(())
    }
    fn logic(&mut self, s: &mut State, ctx: &mut Context) -> GameResult<()> {
        let dist = s.mouse - s.offset - self.world.player.obj.pos;

        self.hud.update_bars(ctx, &self.world.player)?;

        self.world.player.obj.rot = angle_from_vec(dist);

        // Center the camera on the player
        let p = self.world.player.obj.pos;
        s.focus_on(p);
        Ok(())
    }

    fn draw(&mut self, s: &State, ctx: &mut Context) -> GameResult<()> {
        self.world.grid.draw(&self.world.palette, ctx, &s.assets)?;

        self.holes.draw(ctx, Default::default())?;

        for &intel in &self.world.intels {
            let drawparams = graphics::DrawParam {
                dest: intel.into(),
                offset: Point2::new(0.5, 0.5).into(),
                .. Default::default()
            };
            let img = s.assets.get_img(ctx, "common/intel");
            graphics::draw(ctx, &*img, drawparams)?;
        }
        for decal in &self.world.decals {
            decal.draw(ctx, &s.assets, WHITE)?;
        }

        for pickup in &self.world.pickups {
            let drawparams = graphics::DrawParam {
                dest: pickup.pos.into(),
                offset: Point2::new(0.5, 0.5).into(),
                .. Default::default()
            };
            let img = s.assets.get_img(ctx, pickup.pickup_type.spr);
            graphics::draw(ctx, &*img, drawparams)?;
        }
        for wep in &self.world.weapons {
            let drawparams = graphics::DrawParam {
                dest: wep.pos.into(),
                offset: Point2::new(0.5, 0.5).into(),
                .. Default::default()
            };
            let img = s.assets.get_img(ctx, &wep.weapon.entity_sprite);
            graphics::draw(ctx, &*img, drawparams)?;
        }

        self.world.player.draw_player(ctx, &s.assets)?;

        for enemy in &self.world.enemies {
            enemy.draw(ctx, &s.assets, WHITE)?;
        }
        for bullet in &self.world.bullets {
            bullet.draw(ctx, &s.assets)?;
        }
        for grenade in &self.world.grenades {
            grenade.draw(ctx, &s.assets)?;
        }

        Ok(())
    }
    fn draw_hud(&mut self, s: &State, ctx: &mut Context) -> GameResult<()> {
        self.hud.draw(ctx)?;

        self.hp_text.draw_text(ctx)?;
        self.arm_text.draw_text(ctx)?;
        self.reload_text.draw_text(ctx)?;
        self.wep_text.draw_text(ctx)?;
        self.status_text.draw_center(ctx)?;

        {
            let drawparams = DrawParam::from(([104., 2.],));
            let img = s.assets.get_img(ctx, "weapons/knife");
            graphics::draw(ctx, &*img, drawparams)?;
        }
        if let Some(holster_wep) = &self.world.player.wep.holster {
            let drawparams = DrawParam::from(([137., 2.],));
            let img = s.assets.get_img(ctx, &holster_wep.weapon.entity_sprite);
            graphics::draw(ctx, &*img, drawparams)?;
        }
        if let Some(holster_wep) = &self.world.player.wep.holster2 {
            let drawparams = DrawParam::from(([104., 35.],));
            let img = s.assets.get_img(ctx, &holster_wep.weapon.entity_sprite);
            graphics::draw(ctx, &*img, drawparams)?;
        }
        if let Some(sling_wep) = &self.world.player.wep.sling {
            let drawparams = DrawParam::from(([153., 35.],)).offset(Point2::new(0.5, 0.));
            let img = s.assets.get_img(ctx, &sling_wep.weapon.entity_sprite);
            graphics::draw(ctx, &*img, drawparams)?;
        }
        let selection = Mesh::new_rectangle(ctx, DrawMode::stroke(2.), RECTS[self.world.player.wep.active as u8 as usize], Color{r: 1., g: 1., b: 0., a: 1.})?;
        graphics::draw(ctx, &selection, DrawParam::default())?;

        let drawparams = graphics::DrawParam {
            dest: s.mouse.into(),
            offset: Point2::new(0.5, 0.5).into(),
            color: RED,
            .. Default::default()
        };
        let img = s.assets.get_img(ctx, "common/crosshair");
        graphics::draw(ctx, &*img, drawparams)
    }
    fn event_up(&mut self, s: &mut State, ctx: &mut Context, event: Event) {
        use self::KeyCode::*;
        match event {
            Key(Q) => self.world.player.wep.switch(self.world.player.wep.last_active),
            Key(Key1) | Key(Numpad1) => self.world.player.wep.switch(ActiveSlot::Knife),
            Key(Key2) | Key(Numpad2) => self.world.player.wep.switch(ActiveSlot::Holster),
            Key(Key3) | Key(Numpad3) => self.world.player.wep.switch(ActiveSlot::Holster2),
            Key(Key4) | Key(Numpad4) => self.world.player.wep.switch(ActiveSlot::Sling),
            Key(G) => {
                if let Some(wep) = self.world.player.wep.take_active() {
                    self.world.weapons.push(wep.into_drop(self.world.player.obj.pos));
                }
            }
            Key(R) => {
                if let Some(wep) = self.world.player.wep.get_active_mut() {
                    wep.reload(ctx, &mut s.mplayer).unwrap()
                } else {
                    let weapon = &weapon::WEAPONS["glock"];
                    self.world.bullets.push(Bullet{obj: self.world.player.obj.clone(), vel: Vector2::new(weapon.bullet_speed, 0.), weapon});
                }
            },
            Key(F) => {
                if let Some(i) = self.cur_pickup {
                    if let Some(new_drop) = self.world.player.wep.add_weapon(WeaponInstance::from_drop(self.world.weapons.remove(i))) {
                        self.world.weapons.push(new_drop.into_drop(self.world.player.obj.pos));
                    }
                    self.cur_pickup = None;
                }
            },
            Mouse(MouseButton::Left) | Key(Space) => {
                if let Some(wep) = self.world.player.wep.get_active_mut() {
                    if let Some(bm) = wep.shoot(ctx, &mut s.mplayer).unwrap() {
                        let pos = self.world.player.obj.pos + 20. * angle_to_vec(self.world.player.obj.rot);
                        let mut bul = Object::new(pos);
                        bul.rot = self.world.player.obj.rot;

                        for bullet in bm.make(bul) {
                            self.world.bullets.push(bullet);
                        }
                    }
                } else {
                    // TODO do knives with bullets too
                    let player = &mut self.world.player;
                    let mut backstab = false;
                    let mut dead = None;

                    for (i, enemy) in self.world.enemies.iter_mut().enumerate() {
                        let dist = player.obj.pos-enemy.pl.obj.pos;
                        let dist_len = dist.norm();
                        if dist_len < 44. {
                            backstab = angle_to_vec(enemy.pl.obj.rot).dot(&dist) / dist_len < COS_45_D;

                            self.world.decals.push(new_blood(enemy.pl.obj.clone()));
                            enemy.pl.health.weapon_damage(if backstab { 165. } else { 33. }, 0.92);
                            if enemy.pl.health.is_dead() {
                                dead = Some(i);
                                break;
                            }
                        }
                    }
                    if let Some(i) = dead {
                        s.mplayer.play(ctx, "death").unwrap();

                        let Enemy{pl: Player{wep, obj: Object{pos, ..}, ..}, ..}
                            = self.world.enemies.remove(i);
                        for wep in wep {
                            self.world.weapons.push(wep.into_drop(pos));
                        }
                    }

                    s.mplayer.play(ctx, if backstab {"shuk"} else {"hling"}).unwrap();
                }
            }
            _ => (),
        }
    }

    fn event_down(&mut self, s: &mut State, ctx: &mut Context, event: Event) {
        match event {
            Mouse(MouseButton::Right) => {
                if let Some(gm) = self.world.player.wep.utilities.cook_grenade(ctx, &mut s.mplayer).unwrap() {
                    let pos = self.world.player.obj.pos + 20. * angle_to_vec(self.world.player.obj.rot);
                    let mut gren = Object::new(pos);
                    gren.rot = self.world.player.obj.rot;

                    self.world.grenades.push(gm.make(gren));
                }
            }
            _ => (),
        }
    }

    fn get_world(&self) -> Option<&World> {
        Some(&self.world)
    }
    fn get_mut_world(&mut self) -> Option<&mut World> {
        Some(&mut self.world)
    }
}

#[derive(Debug)]
pub struct Hud {
    hud_bar: Mesh,
    hp_bar: Mesh,
    armour_bar: Mesh,
    loading_bar: Mesh,
}

const RECTS: [Rect; 4] = [
    Rect{x:104.,y:2.,h: 32., w: 32.},
    Rect{x:137.,y:2.,h: 32., w: 32.},
    Rect{x:104.,y:35.,h: 32., w: 32.},
    Rect{x:137.,y:35.,h: 32., w: 32.}
];

impl Hud {
    pub fn new(ctx: &mut Context) -> GameResult<Self> {
        let hud_bar = MeshBuilder::new()
            .rectangle(DrawMode::fill(), Rect{x: 1., y: 1., w: 102., h: 26.}, graphics::BLACK)
            .rectangle(DrawMode::fill(), Rect{x: 1., y: 29., w: 102., h: 26.}, graphics::BLACK)
            .rectangle(DrawMode::fill(), Rect{x: 1., y: 57., w: 102., h: 26.}, graphics::BLACK)
            .rectangle(DrawMode::fill(), Rect{x:104.,y:2.,h: 32., w: 32.}, Color{r: 0.5, g: 0.5, b: 0.5, a: 1.})
            .rectangle(DrawMode::fill(), Rect{x:137.,y:2.,h: 32., w: 32.}, Color{r: 0.5, g: 0.5, b: 0.5, a: 1.})
            .rectangle(DrawMode::fill(), Rect{x:104.,y:35.,h: 32., w: 32.}, Color{r: 0.5, g: 0.5, b: 0.5, a: 1.})
            .rectangle(DrawMode::fill(), Rect{x:137.,y:35.,h: 32., w: 32.}, Color{r: 0.5, g: 0.5, b: 0.5, a: 1.})
            .build(ctx)?;

        let hp_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 2., w: 0., h: 24.}, GREEN)?;
        let armour_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 30., w: 0., h: 24.}, BLUE)?;
        let loading_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 58., w: 0., h: 24.}, RED)?;

        Ok(Hud{
            hud_bar,
            hp_bar,
            armour_bar,
            loading_bar,
        })
    }
    pub fn update_bars(&mut self, ctx: &mut Context, p: &Player) -> GameResult<()> {
        self.hp_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 2., w: p.health.hp.limit(0., 100.), h: 24.}, GREEN)?;
        self.armour_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 30., w: p.health.armour.limit(0., 100.), h: 24.}, BLUE)?;
        self.loading_bar = Mesh::new_rectangle(ctx, DrawMode::fill(), Rect{x: 2., y: 58., w: p.wep.get_active().map(|m| m.loading_time).unwrap_or(0.).limit(0., 1.)*100., h: 24.}, RED)?;

        Ok(())
    }
    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        self.hud_bar.draw(ctx, Default::default())?;
        self.hp_bar.draw(ctx, Default::default())?;
        self.armour_bar.draw(ctx, Default::default())?;
        self.loading_bar.draw(ctx, Default::default())
    }
}