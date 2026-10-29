//! The saved game (MISC.S DOSAVEGAME, TOPCTRL.S ResumeGame).
//!
//! As in the original, a save holds the level, the kid's strength when it
//! began, the clock and the next time message; resuming starts that level
//! from its beginning. Saving works from level 3 to 13 (FirstSideB, and
//! SDLPoP's limits). Ctrl-G saves as the original did, and reaching one of
//! those levels saves too, so quitting never loses more than the level
//! being played. Ctrl-L resumes, from the opening scene, the demo or the
//! end of a game, as SDLPoP's Ctrl-L does from the title.

use crate::game::Game;
use std::path::PathBuf;

/// The levels a game can be saved on.
pub const LEVELS: std::ops::RangeInclusive<u8> = 3..=13;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Save {
    pub level: u8,
    /// origstrength
    pub strength: u8,
    pub frame_count: u32,
    pub next_time_msg: usize,
}

impl Save {
    /// The game as it is now, or `None` where saving isn't allowed.
    pub fn of(g: &Game) -> Option<Save> {
        LEVELS.contains(&g.level.number).then_some(Save {
            level: g.level.number,
            strength: g.orig_strength,
            frame_count: g.frame_count,
            next_time_msg: g.next_time_msg,
        })
    }

    /// ResumeGame: strength and clock back, the level from its start,
    /// showing the time left.
    pub fn resume(&self) -> Game {
        let mut g = Game::new(1);
        g.max_strength = self.strength;
        g.frame_count = self.frame_count;
        g.next_time_msg = self.next_time_msg;
        g.start_level(self.level, true);
        g.time_request = 1;
        g
    }

    fn to_text(self) -> String {
        format!(
            "level {}\nstrength {}\nframes {}\nnextmsg {}\n",
            self.level, self.strength, self.frame_count, self.next_time_msg
        )
    }

    fn from_text(s: &str) -> Option<Save> {
        let mut v = [None::<u32>; 4];
        for line in s.lines() {
            let (k, n) = line.split_once(' ')?;
            let i = ["level", "strength", "frames", "nextmsg"].iter().position(|w| *w == k)?;
            v[i] = n.trim().parse().ok();
        }
        let save = Save {
            level: u8::try_from(v[0]?).ok()?,
            strength: u8::try_from(v[1]?).ok().filter(|s| (1..=10).contains(s))?,
            frame_count: v[2]?,
            next_time_msg: v[3]? as usize,
        };
        LEVELS.contains(&save.level).then_some(save)
    }
}

/// Where the save is kept: the user's data folder.
fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join("prince-of-braille").join("save"))
}

pub fn load() -> Option<Save> {
    Save::from_text(&std::fs::read_to_string(path()?).ok()?)
}

/// Write the save; false if it couldn't be.
pub fn store(s: Save) -> bool {
    let Some(p) = path() else { return false };
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    match std::fs::write(&p, s.to_text()) {
        Ok(()) => {
            log::info!("game saved: {s:?}");
            true
        }
        Err(e) => {
            log::warn!("could not save the game to {}: {e}", p.display());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_round_trips() {
        let s = Save { level: 7, strength: 5, frame_count: 20_000, next_time_msg: 6 };
        assert_eq!(Save::from_text(&s.to_text()), Some(s));
        assert_eq!(Save::from_text("level 2\nstrength 3\nframes 0\nnextmsg 0\n"), None);
        assert_eq!(Save::from_text("rubbish"), None);
    }

    #[test]
    fn resuming_starts_the_level_with_the_saved_strength_and_clock() {
        let s = Save { level: 5, strength: 6, frame_count: 12_345, next_time_msg: 4 };
        let g = s.resume();
        assert_eq!(g.level.number, 5);
        assert_eq!((g.kid_strength, g.max_strength, g.orig_strength), (6, 6, 6));
        assert_eq!((g.frame_count, g.next_time_msg), (12_345, 4));
        assert_eq!(Save::of(&g), Some(s));
    }

    #[test]
    fn a_restart_takes_back_a_potion_drunk_before_dying() {
        let mut g = Game::new(3);
        assert_eq!(g.orig_strength, 3);
        g.boostmeter();
        assert_eq!(g.max_strength, 4);
        g.restart_level();
        assert_eq!((g.max_strength, g.kid_strength), (3, 3));
        // but the strength gained is kept on the next level
        g.boostmeter();
        g.start_level(4, true);
        assert_eq!((g.orig_strength, g.max_strength), (4, 4));
    }
}
