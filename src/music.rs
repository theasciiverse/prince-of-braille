//! Music: the DOS version's songs as MP3s (from PrinceJS, Unlicense),
//! cued where the Apple code cued its songs. A cue starts the song at once
//! over the game.

use crate::game::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Song {
    /// the title's main theme
    PrologueA,
    /// "In the Sultan's absence"
    PrologueB,
    Princess,
    Jaffar,
    Heartbeat,
    Danger,
    Accident,
    SmallPotion,
    Victory,
    Prince,
    HeroicDeath,
    BigPotion,
    Shadow,
    Float,
    Timer,
    Tragic,
    MeetJaffar,
    JaffarDead,
    Embrace,
    /// the end, after the embrace
    Epilogue,
}

impl Song {
    pub fn mp3(self) -> &'static [u8] {
        match self {
            Song::PrologueA => include_bytes!("../assets/music/01_Prologue_A.mp3"),
            Song::PrologueB => include_bytes!("../assets/music/02_Prologue_B.mp3"),
            Song::Princess => include_bytes!("../assets/music/03_Princess.mp3"),
            Song::Jaffar => include_bytes!("../assets/music/04_Jaffar.mp3"),
            Song::Heartbeat => include_bytes!("../assets/music/05_Heartbeat.mp3"),
            Song::Danger => include_bytes!("../assets/music/06_Danger.mp3"),
            Song::Accident => include_bytes!("../assets/music/07_Accident.mp3"),
            Song::SmallPotion => include_bytes!("../assets/music/08_Potion_1.mp3"),
            Song::Victory => include_bytes!("../assets/music/09_Victory.mp3"),
            Song::Prince => include_bytes!("../assets/music/11_Prince.mp3"),
            Song::HeroicDeath => include_bytes!("../assets/music/13_Heroic_Death.mp3"),
            Song::BigPotion => include_bytes!("../assets/music/14_Potion_2.mp3"),
            Song::Shadow => include_bytes!("../assets/music/15_The_Shadow.mp3"),
            Song::Float => include_bytes!("../assets/music/16_Float.mp3"),
            Song::Timer => include_bytes!("../assets/music/17_Timer.mp3"),
            Song::Tragic => include_bytes!("../assets/music/18_Tragic_End.mp3"),
            Song::MeetJaffar => include_bytes!("../assets/music/19_Jaffar_2.mp3"),
            Song::JaffarDead => include_bytes!("../assets/music/20_Jaffar_Dead.mp3"),
            Song::Embrace => include_bytes!("../assets/music/21_Embrace.mp3"),
            Song::Epilogue => include_bytes!("../assets/music/22_Epilogue.mp3"),
        }
    }
}

impl Game {
    /// CUESONG: play `s` (main hands it to the audio after this frame).
    pub fn cuesong(&mut self, s: Song) {
        if self.level.number == 0 {
            return; // no music in the demo
        }
        log::info!("song {s:?}");
        self.song_start = Some(s);
    }

    /// deathsong: what the kid's death sounds like.
    pub fn deathsong(&mut self) {
        let s = if self.shad.id == 1 && self.shad.face != crate::guard::NO_CHAR {
            Song::Shadow
        } else if self.heroic {
            Song::HeroicDeath
        } else {
            Song::Accident
        };
        self.cuesong(s);
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
            self.cuesong(Song::Danger);
            self.exitopen = 77;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source;

    #[test]
    fn every_song_decodes() {
        use Song::*;
        for s in [
            Princess, Jaffar, Heartbeat, Danger, Accident, SmallPotion, Victory, Prince, HeroicDeath, BigPotion,
            Shadow, Float, Timer, Tragic, MeetJaffar, JaffarDead, Embrace,
        ] {
            let d = rodio::Decoder::try_from(std::io::Cursor::new(s.mp3())).expect("decodes");
            let rate = d.sample_rate().get();
            let n = d.take(rate as usize).count();
            assert!(n > 1000, "{s:?}: {n} samples");
        }
    }
}
