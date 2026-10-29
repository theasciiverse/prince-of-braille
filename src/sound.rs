//! Sound effects: the DOS version's, as MP3s from PrinceJS (Unlicense),
//! played where SDLPoP plays them (see `Sound`). They are decoded once at
//! start-up and mixed through rodio over the music, the way the other
//! Asciiverse games do it.
//!
//! Without an audio device, or with `--mute`, every call does nothing.

use crate::game::Sound;
use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Player, Source};
use std::time::{Duration, Instant};

/// The MP3 for an effect; the loose floor has three.
fn mp3s(s: Sound) -> &'static [&'static [u8]] {
    macro_rules! sfx {
        ($($f:literal),+) => { &[$(include_bytes!(concat!("../assets/sfx/", $f, ".mp3"))),+] };
    }
    match s {
        Sound::FellToDeath => sfx!("01_Free_fall_land"),
        Sound::Falling => sfx!("28_Falling_floor_lands"),
        Sound::TileCrash => sfx!("02_Loose_floor_lands"),
        Sound::Button => sfx!("19_Floor_button"),
        Sound::GateClosing => sfx!("04_Gate_coming_down_slow"),
        Sound::GateOpening => sfx!("05_Gate_rising"),
        Sound::GateSlam => sfx!("06_Gate_reaches_bottom_clang"),
        Sound::GateStop => sfx!("07_Gate_stops_at_top"),
        Sound::Bump => sfx!("08_Bump_into_wall_soft"),
        Sound::Grab => sfx!("09_Bump_into_wall_hard"),
        Sound::SwordClash => sfx!("10_Sword_clash"),
        Sound::SwordMoving => sfx!("11_Stab_air"),
        Sound::GuardHurt => sfx!("12_Stab_opponent"),
        Sound::KidHurt => sfx!("13_Stabbed_by_opponent"),
        Sound::LevelDoorClosing => sfx!("29_Entrance_door_closes"),
        Sound::LevelDoorSliding => sfx!("30_Exit_door_opening"),
        Sound::MediumLand => sfx!("14_Medium_landing_oof"),
        Sound::SoftLand => sfx!("15_Soft_landing"),
        Sound::Drink => sfx!("31_Drink_potion_glug_glug"),
        Sound::DrawSword => sfx!("16_Unsheathe_sword"),
        Sound::LooseShake => sfx!("03_Loose_floor_shakes", "18_Loose_floor_shakes_2", "17_Loose_floor_shakes_3"),
        Sound::Footstep => sfx!("20_Footsteps"),
        Sound::SkelAlive => sfx!("21_Bones_leap_to_life"),
        Sound::Mirror => sfx!("22_Mirror"),
        Sound::Chomped => sfx!("23_Halved_by_chopper"),
        Sound::Chomper => sfx!("24_Slicer_blades_clash"),
        Sound::Spiked => sfx!("33_Spiked_by_spikes"),
        Sound::Spikes => sfx!("26_Impaled_by_spikes"),
        Sound::PrincessDoor => sfx!("27_Door_squeak"),
    }
}

const ALL: [Sound; 29] = [
    Sound::FellToDeath,
    Sound::Falling,
    Sound::TileCrash,
    Sound::Button,
    Sound::GateClosing,
    Sound::GateOpening,
    Sound::GateSlam,
    Sound::GateStop,
    Sound::Bump,
    Sound::Grab,
    Sound::SwordClash,
    Sound::SwordMoving,
    Sound::GuardHurt,
    Sound::KidHurt,
    Sound::LevelDoorClosing,
    Sound::LevelDoorSliding,
    Sound::MediumLand,
    Sound::SoftLand,
    Sound::Drink,
    Sound::DrawSword,
    Sound::LooseShake,
    Sound::Footstep,
    Sound::SkelAlive,
    Sound::Mirror,
    Sound::Chomped,
    Sound::Chomper,
    Sound::Spiked,
    Sound::Spikes,
    Sound::PrincessDoor,
];

fn decode(mp3: &'static [u8]) -> Option<SamplesBuffer> {
    let d = rodio::Decoder::try_from(std::io::Cursor::new(mp3)).ok()?;
    let (ch, rate) = (d.channels(), d.sample_rate());
    let v: Vec<f32> = d.collect();
    (!v.is_empty()).then(|| SamplesBuffer::new(ch, rate, v))
}

pub struct Audio {
    // the sink must outlive everything playing on its mixer
    sink: Option<MixerDeviceSink>,
    /// each effect's decoded takes, in ALL's order
    bank: Vec<Vec<SamplesBuffer>>,
    /// the loose floor's last take, so the next one differs (SDLPoP
    /// last_loose_sound)
    last_loose: usize,
    /// a xorshift for picking the loose floor's take (not the game's RNG,
    /// so sound never changes how a game or the demo plays)
    rng: u32,
    /// when the exit door's slide ends: SDLPoP doesn't restart it while
    /// it plays, so asked for every frame it is heard once
    sliding_until: Option<Instant>,
    pub muted: bool,
    /// the music track, so a song can be stopped
    music: Option<Player>,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        let sink = if muted {
            None
        } else {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    Some(s)
                }
                Err(e) => {
                    log::warn!("no audio: {e}");
                    None
                }
            }
        };
        let bank = if sink.is_some() {
            ALL.iter().map(|&s| mp3s(s).iter().filter_map(|m| decode(m)).collect()).collect()
        } else {
            Vec::new()
        };
        let music = sink.as_ref().map(|s| Player::connect_new(s.mixer()));
        Audio { sink, bank, last_loose: 0, rng: 0x9e37_79b9, sliding_until: None, muted, music }
    }

    /// Start `song`, ending whatever song was playing.
    pub fn start_song(&mut self, song: crate::music::Song) {
        if self.muted {
            return;
        }
        let Some(m) = &self.music else { return };
        match rodio::Decoder::try_from(std::io::Cursor::new(song.mp3())) {
            Ok(d) => {
                m.clear();
                m.append(d);
                m.play();
            }
            Err(e) => log::warn!("song {song:?}: {e}"),
        }
    }

    pub fn stop_song(&mut self) {
        if let Some(m) = &self.music {
            m.clear();
        }
    }

    /// PLAYBACK: this frame's sounds, each once.
    pub fn play(&mut self, sounds: &[Sound]) {
        if self.muted {
            return;
        }
        let Some(sink) = &self.sink else { return };
        for (n, &s) in sounds.iter().enumerate() {
            if sounds[..n].contains(&s) {
                continue;
            }
            let i = ALL.iter().position(|&a| a == s).unwrap_or(0);
            let Some(takes) = self.bank.get(i).filter(|t| !t.is_empty()) else { continue };
            let take = match s {
                Sound::LooseShake => {
                    // one of the other takes at random, as SDLPoP's loose_shake
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 17;
                    self.rng ^= self.rng << 5;
                    let k = takes.len();
                    self.last_loose = (self.last_loose + 1 + self.rng as usize % (k - 1).max(1)) % k;
                    &takes[self.last_loose]
                }
                _ => &takes[0],
            };
            if s == Sound::LevelDoorSliding {
                let now = Instant::now();
                if self.sliding_until.is_some_and(|t| now < t) {
                    continue;
                }
                let len = take.total_duration().unwrap_or(Duration::ZERO);
                self.sliding_until = Some(now + len);
            }
            sink.mixer().add(take.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_decodes() {
        for s in ALL {
            for m in mp3s(s) {
                let b = decode(m).unwrap_or_else(|| panic!("{s:?} decodes"));
                assert!(b.total_duration().unwrap() > Duration::from_millis(40), "{s:?}");
            }
        }
    }
}
