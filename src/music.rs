//! Song cues (SPECIALK.S CUESONG, TOPCTRL.S songcues and deathsong,
//! AUTO.S mirrmusic). A cued song waits, for at most its count of frames,
//! until nothing on the screen is moving; then the game stands still while
//! it plays, only the torches burning, as on the Apple. With the music off
//! a cue is simply forgotten, as the original's silent MPLAY did. Unlike
//! the original, the button cuts a song short.

use crate::game::*;
use crate::level::obj;
use crate::sid::Song;

/// TOPCTRL.S static?: positions a song may freeze a character in.
fn still(posn: u8) -> bool {
    matches!(posn, 0 | 15 | 229 | 109 | 171 | 166 | 185 | 177 | 178)
}

impl Game {
    /// CUESONG: song `s`, to be played within `count` frames.
    pub fn cuesong(&mut self, s: Song, count: u8) {
        if self.music_on {
            log::info!("song cue {s:?}");
            self.song_cue = Some((s, count));
        }
    }

    /// deathsong: what the kid's death sounds like.
    pub fn deathsong(&mut self) {
        let s = if self.shad.id == 1 && self.shad.face != crate::guard::NO_CHAR {
            Song::Shadow
        } else if self.heroic {
            Song::Heroic
        } else {
            Song::Accid
        };
        self.cuesong(s, 255);
    }

    /// songcues: start the cued song once the scene is still enough.
    pub fn songcues(&mut self) {
        let Some((s, count)) = self.song_cue else { return };
        if count == 0 {
            self.song_cue = None;
            return;
        }
        self.song_cue = Some((s, count - 1));
        // once the kid has reached the stairs, play only one song
        if self.kid.posn == 0 && self.next_level {
            return;
        }
        if !still(self.kid.posn) {
            return;
        }
        let sh = &self.shad;
        if sh.face != crate::guard::NO_CHAR && sh.scrn == self.vis_scrn && !still(sh.posn) {
            return;
        }
        // nothing fast-moving that would look wrong frozen
        let busy = self.trobs.iter().any(|t| {
            t.scrn == self.vis_scrn
                && t.loc < 30
                && matches!(
                    self.level.bluetype[crate::level::Level::index(t.scrn, (t.loc % 10) as i32, (t.loc / 10) as i32)]
                        & 0x1f,
                    obj::LOOSE | obj::SPIKES | obj::SLICER
                )
        });
        if busy || !self.mobs.is_empty() || self.lightning != 0 {
            return;
        }
        if self.mergetimer > 0 || self.chg_kid_str != 0 || self.chg_opp_str != 0 {
            return;
        }
        let frames = (crate::sid::seconds(s) * FPS as f64).ceil() as u32;
        log::info!("song {s:?} plays for {frames} frames");
        self.song_cue = None;
        self.song = Some((s, frames));
        self.song_start = Some(s);
    }

    /// One frame while a song plays: only the torches burn. Returns false
    /// once the song is over (or the button cut it short).
    pub fn song_tick(&mut self, c: Controls) -> bool {
        let Some((s, left)) = self.song else { return false };
        if left == 0 || (c.btn && !self.song_btn) {
            self.song = None;
            self.song_stop = true;
            return false;
        }
        self.song_btn = c.btn;
        self.song = Some((s, left - 1));
        self.burn();
        true
    }

    /// BURN: the torches on the visible screen flicker on.
    fn burn(&mut self) {
        for k in 0..self.trobs.len() {
            let t = self.trobs[k];
            if t.scrn != self.vis_scrn || t.loc >= 30 || t.direc < 0 {
                continue;
            }
            let i = crate::level::Level::index(t.scrn, (t.loc % 10) as i32, (t.loc / 10) as i32);
            if self.level.bluetype[i] & 0x1f == obj::TORCH {
                let st = self.level.bluespec[i];
                self.level.bluespec[i] = self.flameframe(st);
            }
        }
    }

    /// mirrmusic (AUTO.S): on level 4, the danger theme as the kid heads
    /// left towards the mirror from the screen to its right.
    pub fn mirrmusic(&mut self) {
        const MIRSCRN_R: u8 = 11;
        if self.level.number == 4
            && self.exitopen != 0
            && self.exitopen != 77
            && self.kid.block_y == 0
            && self.kid.scrn == MIRSCRN_R
        {
            self.cuesong(Song::Danger, 50);
            self.exitopen = 77;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cue_waits_for_stillness_and_then_freezes_the_game() {
        let mut g = Game::new(2);
        g.music_on = true;
        for _ in 0..10 {
            g.tick(Controls::default());
        }
        g.cuesong(Song::Sword, 25);
        g.tick(Controls::default());
        let (s, frames) = g.song.expect("the kid stands still, so the song starts");
        assert_eq!(s, Song::Sword);
        assert_eq!(g.song_start, Some(Song::Sword));
        let ticks = g.ticks;
        let x = g.kid.x;
        for _ in 0..5 {
            g.tick(Controls { x: 1, ..Controls::default() });
        }
        assert_eq!(g.kid.x, x, "nothing moves while the song plays");
        assert!(g.ticks > ticks);
        for _ in 0..frames + 2 {
            g.tick(Controls::default());
        }
        assert!(g.song.is_none());
    }

    #[test]
    fn with_the_music_off_a_cue_is_forgotten() {
        let mut g = Game::new(2);
        g.cuesong(Song::Sword, 25);
        g.tick(Controls::default());
        assert!(g.song.is_none() && g.song_cue.is_none());
    }
}
