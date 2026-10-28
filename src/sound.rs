//! Sound effects: SOUND.S's speaker tones, rebuilt sample for sample.
//!
//! The Apple II speaker is one bit: every `bit $c030` flips the cone.
//! SOUND.S's `tone` flips it `duration` times with a delay loop between
//! flips, so each effect is a short square wave whose half-period is the
//! cycle count of that loop at 1.023 MHz. The effects are rendered into
//! buffers once at start-up and played through rodio, the way the other
//! Asciiverse games do it. Music (the song cues) is not played.
//!
//! Without an audio device, or with `--mute`, every call does nothing.

use crate::game::Sound;
use rodio::buffer::SamplesBuffer;
use rodio::{MixerDeviceSink, Player};
use std::num::NonZero;

const SR: u32 = 44_100;
/// Apple II CPU clock.
const CLOCK: f64 = 1_022_727.0;
const LEVEL: f32 = 0.25;

/// One `tone` call: pitch (the 16-bit Y-X loop count) and duration (the
/// number of speaker flips).
type Tone = (u16, u8);

/// The tones each effect plays (SOUND.S, DoPlateDown ... DoJawsClash).
fn tones(s: Sound) -> &'static [Tone] {
    const SPLAT: &[Tone] = &[(1000, 3)];
    const CLASH: &[Tone] = &[(15, 50)];
    match s {
        Sound::PlateDown | Sound::GateDown => &[(70, 4)],
        Sound::PlateUp => &[(90, 4)],
        Sound::JawsClash => &[(10, 50)],
        Sound::SwordClash => CLASH,
        Sound::Splat | Sound::MirrorCrack | Sound::LooseCrash => SPLAT,
        Sound::SmackWall | Sound::Impaled | Sound::GateSlam => SPLAT,
        Sound::Footstep => &[(35, 3)],
        Sound::RaisingExit => &[(40, 6)],
        Sound::RaisingGate => &[(20, 2)],
        // music cues in the original, not speaker effects
        Sound::Drink | Sound::Upstairs => &[],
    }
}

/// CPU cycles between two flips in `tone` for a pitch: the inner loop
/// (iny, cpy abs, bcc) runs `lo` times, the middle loop `hi` times.
fn half_period(pitch: u16) -> f64 {
    let lo = (pitch & 0xff) as f64;
    let hi = (pitch >> 8) as f64;
    let inner = 9.0 * lo.max(1.0) - 1.0;
    let middle = (inner + 2.0 + 9.0) * hi.max(1.0) - 1.0;
    4.0 + 2.0 + middle + 7.0
}

/// The speaker waveform for a list of tones.
fn render(t: &[Tone]) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cone = LEVEL;
    for &(pitch, dur) in t {
        let n = (half_period(pitch) / CLOCK * SR as f64).max(1.0);
        let mut acc = 0.0;
        for _ in 0..dur {
            cone = -cone;
            acc += n;
            while (out.len() as f64) < acc {
                out.push(cone);
            }
        }
    }
    // let the cone settle instead of ending on a click
    let tail = out.len().min(64);
    let len = out.len();
    for (i, v) in out[len - tail..].iter_mut().enumerate() {
        *v *= 1.0 - i as f32 / tail as f32;
    }
    out
}

const ALL: [Sound; 15] = [
    Sound::Footstep,
    Sound::SmackWall,
    Sound::Splat,
    Sound::Impaled,
    Sound::GateDown,
    Sound::GateSlam,
    Sound::RaisingGate,
    Sound::RaisingExit,
    Sound::PlateDown,
    Sound::PlateUp,
    Sound::LooseCrash,
    Sound::SwordClash,
    Sound::JawsClash,
    Sound::MirrorCrack,
    Sound::Drink,
];

pub struct Audio {
    // the sink must outlive everything playing on its mixer
    sink: Option<MixerDeviceSink>,
    bank: Vec<Option<SamplesBuffer>>,
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
            ALL.iter()
                .map(|&s| {
                    let v = render(tones(s));
                    (!v.is_empty()).then(|| SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(SR).unwrap(), v))
                })
                .collect()
        } else {
            Vec::new()
        };
        let music = sink.as_ref().map(|s| Player::connect_new(s.mixer()));
        Audio { sink, bank, muted, music }
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

    /// PLAYBACK: this frame's sound table.
    pub fn play(&mut self, sounds: &[Sound]) {
        if self.muted {
            return;
        }
        let Some(sink) = &self.sink else { return };
        for s in sounds {
            let i = ALL.iter().position(|a| a == s).unwrap_or(0);
            if let Some(Some(b)) = self.bank.get(i) {
                sink.mixer().add(b.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_have_the_loop_lengths_of_sound_s() {
        // a footstep: 35 turns of the inner loop between flips, 3 flips
        let f = render(tones(Sound::Footstep));
        let ms = f.len() as f64 * 1000.0 / SR as f64;
        assert!((0.8..1.3).contains(&ms), "footstep {ms} ms");
        // the splat's pitch 1000 is 3 x 232 turns: a low thud
        assert!(half_period(1000) > 15.0 * half_period(35));
        assert!(render(tones(Sound::Drink)).is_empty());
    }
}
