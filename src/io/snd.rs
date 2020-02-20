use std::collections::HashMap;

use ggez::{Context, GameResult};
use ggez::audio::{Source, SoundData, SoundSource};

const EFFECTS_LIMIT: usize = 25;

fn new_source(ctx: &mut Context, data: &SoundData) -> GameResult<Source> {
    Source::from_data(ctx, data.clone()).map(|mut src| {
        src.set_volume(0.01);
        src
    })
}

pub struct MediaPlayer {
    data: HashMap<String, SoundData>,
    // containers for sources
    music_sources: HashMap<String, Source>,
    effects: Vec<Source>,
}

impl Default for MediaPlayer {
    #[inline]
    fn default() -> Self {
        MediaPlayer::new()
    }
}

impl MediaPlayer {
    #[inline]
    pub fn new() -> Self {
        MediaPlayer {
            effects: Vec::with_capacity(10),
            music_sources: HashMap::new(),
            data: HashMap::with_capacity(24),
        }
    }
    pub fn add_effect(&mut self, ctx: &mut Context, s: &str) -> GameResult<&mut SoundData> {
        let data = SoundData::new(ctx, format!("/sounds/{}.wav", s))?;
        self.data.insert(s.to_owned(), data);
        Ok(self.data.get_mut(s).unwrap())
    }
    pub fn register_music<S: Into<String>>(&mut self, ctx: &mut Context, s: S, repeat: bool) -> GameResult<()> {
        let s = s.into();

        let data = SoundData::new(ctx, format!("/sounds/{}.ogg", s))?;
        self.data.insert(s.clone(), data);

        let cache = self.new_cache(ctx, &s, repeat)?;
        self.music_sources.insert(s, cache);
        Ok(())
    }
    pub fn play(&mut self, ctx: &mut Context, s: &str) -> GameResult<()> {
        let snd;

        if let Some(music) = self.music_sources.get_mut(s) {
            return music.play();
        } else if let Some(s) = self.data.get(s) {
            snd = s;
        } else {
            snd = self.add_effect(ctx, s)?;
        }
        let mut src = new_source(ctx, snd)?;
        src.play()?;

        self.clear_effects();

        if self.effects.len() < EFFECTS_LIMIT {
            self.effects.push(src);
        }
        Ok(())
    }
    fn clear_effects(&mut self) {
        self.effects.retain(|src| src.playing());
    }
    fn new_cache(&self, ctx: &mut Context, s: &str, repeat: bool) -> GameResult<Source> {
        Source::from_data(ctx, self.data[s].clone())
            .map(|mut src| {
                src.set_volume(0.05);
                src.set_repeat(repeat);
                src
            })
    }
    pub fn stop(&mut self, ctx: &mut Context, s: &str) -> GameResult<()> {
        let repeat;
        if let Some(music_source) = self.music_sources.get_mut(s) {
            repeat = music_source.repeat();
            music_source.stop();
        } else {
            panic!("{:?} can't be stopped", s);
        }

        if repeat {
            self.music_sources.insert(s.to_owned(), self.new_cache(ctx, s, true)?);
        }
        Ok(())
    }
}
