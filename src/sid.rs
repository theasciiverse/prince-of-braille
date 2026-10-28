//! The music, from the C64 port (the Apple II music module is not in the
//! released source). `tools/c64music.py` ran that port's own driver and
//! wrote down, call by call, what it put into the SID chip; this plays
//! those registers back through a small SID-like synth: three voices of
//! triangle, saw, pulse or noise, each with the SID's ADSR envelope. The
//! songs leave the filter alone, so there is none here.

/// assets/c64/MUSIC.BIN, see tools/c64music.py
static DATA: &[u8] = include_bytes!("../assets/c64/MUSIC.BIN");

/// PAL C64 clock, Hz
const CLOCK: f64 = 985_248.0;
/// SID attack times, ms, by register value
const ATTACK_MS: [f64; 16] = [2., 8., 16., 24., 38., 56., 68., 80., 100., 250., 500., 800., 1000., 3000., 5000., 8000.];
/// SID decay and release times, ms
const DECAY_MS: [f64; 16] =
    [6., 24., 48., 72., 114., 168., 204., 240., 300., 750., 1500., 2400., 3000., 9000., 15000., 24000.];

/// The Apple's songs (SOUNDNAMES.S and MASTER.S) and the C64 port's song
/// numbers for them. The port numbers the princess's-room and title songs
/// as the Apple's title set does, and the in-game songs 16 above their
/// Apple numbers (its Timer and Heartbeat sit in both places).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Song {
    Accid,
    Heroic,
    Danger,
    Sword,
    Rejoin,
    Shadow,
    Vict,
    Stairs,
    Upstairs,
    Potion,
    ShortPot,
    Timer,
    Tragic,
    Embrace,
    Heartbeat,
    Princess,
    Vizier,
    Buildup,
    Magic,
}

impl Song {
    pub fn c64(self) -> u8 {
        match self {
            Song::Accid => 17,
            Song::Heroic => 18,
            Song::Danger => 19,
            Song::Sword => 20,
            Song::Rejoin => 21,
            Song::Shadow => 22,
            Song::Vict => 23,
            Song::Stairs => 24,
            Song::Upstairs => 25,
            Song::Potion => 27,
            Song::ShortPot => 28,
            Song::Timer => 13,
            // the port's 30, the in-game Tragic, is the same as its 7
            Song::Tragic => 7,
            Song::Embrace => 15,
            Song::Heartbeat => 16,
            Song::Princess => 7,
            Song::Vizier => 9,
            Song::Buildup => 10,
            Song::Magic => 11,
        }
    }
}

/// One song's register changes: (call, register, value).
fn records(song: u8) -> Option<&'static [u8]> {
    let mut o = 1;
    for _ in 0..*DATA.first()? {
        let n = *DATA.get(o)?;
        let count = u32::from_le_bytes(DATA.get(o + 1..o + 5)?.try_into().ok()?) as usize;
        let body = DATA.get(o + 5..o + 5 + count * 4)?;
        if n == song {
            return Some(body);
        }
        o += 5 + count * 4;
    }
    None
}

#[derive(Clone, Copy, Default)]
struct Voice {
    phase: f64,
    noise: f32,
    noise_step: u32,
    lfsr: u32,
    env: f64,
    stage: Stage,
    gate: bool,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Stage {
    #[default]
    Release,
    Attack,
    Decay,
}

/// Seconds the song lasts.
pub fn seconds(song: Song) -> f64 {
    let Some(r) = records(song.c64()) else { return 0.0 };
    let mut timer = 0u16;
    let mut t = 0.0;
    let mut call = 0u16;
    for rec in r.chunks_exact(4) {
        let n = u16::from_le_bytes([rec[0], rec[1]]);
        while call < n {
            t += call_seconds(timer);
            call += 1;
        }
        match rec[2] {
            0x19 => timer = timer & 0xff00 | rec[3] as u16,
            0x1a => timer = timer & 0x00ff | (rec[3] as u16) << 8,
            _ => {}
        }
    }
    t + call_seconds(timer)
}

/// One call of the play routine lasts one period of the CIA timer.
fn call_seconds(timer: u16) -> f64 {
    if timer == 0 {
        1.0 / 50.0
    } else {
        timer as f64 / CLOCK
    }
}

/// The song as mono samples at `sr` Hz.
pub fn render(song: Song, sr: u32) -> Vec<f32> {
    let Some(r) = records(song.c64()) else { return Vec::new() };
    let sr = sr as f64;
    let mut regs = [0u8; 0x1b];
    let mut v = [Voice { lfsr: 0x7ffff8, ..Voice::default() }; 3];
    let mut out = Vec::new();
    let mut i = 0;
    let recs: Vec<&[u8]> = r.chunks_exact(4).collect();
    let last = recs.last().map_or(0, |x| u16::from_le_bytes([x[0], x[1]]));
    let mut owed = 0.0;
    for call in 0..=last {
        while i < recs.len() && u16::from_le_bytes([recs[i][0], recs[i][1]]) == call {
            regs[recs[i][2] as usize] = recs[i][3];
            i += 1;
        }
        let timer = regs[0x19] as u16 | (regs[0x1a] as u16) << 8;
        owed += call_seconds(timer) * sr;
        let n = owed as usize;
        owed -= n as f64;
        for _ in 0..n {
            let mut mix = 0.0f32;
            for (k, voice) in v.iter_mut().enumerate() {
                mix += sample(voice, &regs[k * 7..k * 7 + 7], sr);
            }
            let vol = (regs[0x18] & 0x0f) as f32 / 15.0;
            out.push(mix * vol * 0.22);
        }
    }
    out
}

fn sample(v: &mut Voice, r: &[u8], sr: f64) -> f32 {
    let freq = (r[0] as u32 | (r[1] as u32) << 8) as f64 * CLOCK / 16_777_216.0;
    let pw = (r[2] as u32 | ((r[3] & 0x0f) as u32) << 8) as f64 / 4096.0;
    let ctrl = r[4];
    let (a, d) = (r[5] >> 4, r[5] & 0x0f);
    let (s, rel) = (r[6] >> 4, r[6] & 0x0f);
    // gate edges start the attack or the release
    let gate = ctrl & 1 != 0;
    if gate && !v.gate {
        v.stage = Stage::Attack;
    } else if !gate && v.gate {
        v.stage = Stage::Release;
    }
    v.gate = gate;
    let sus = s as f64 / 15.0;
    match v.stage {
        Stage::Attack => {
            v.env += 1000.0 / (ATTACK_MS[a as usize] * sr);
            if v.env >= 1.0 {
                v.env = 1.0;
                v.stage = Stage::Decay;
            }
        }
        Stage::Decay => {
            if v.env > sus {
                v.env = (v.env - 1000.0 / (DECAY_MS[d as usize] * sr)).max(sus);
            }
        }
        Stage::Release => {
            v.env = (v.env - 1000.0 / (DECAY_MS[rel as usize] * sr)).max(0.0);
        }
    }
    // the test bit holds the oscillator at zero
    if ctrl & 0x08 != 0 {
        v.phase = 0.0;
        return 0.0;
    }
    v.phase += freq / sr;
    let wraps = v.phase as u32;
    v.phase -= wraps as f64;
    let p = v.phase;
    let wave = if ctrl & 0x80 != 0 {
        // noise: a new value sixteen times a cycle, from the SID's LFSR
        let step = (p * 16.0) as u32 + wraps * 16;
        if step != v.noise_step {
            v.noise_step = step;
            let bit = ((v.lfsr >> 22) ^ (v.lfsr >> 17)) & 1;
            v.lfsr = (v.lfsr << 1 | bit) & 0x7f_ffff;
            v.noise = (v.lfsr & 0xff) as f32 / 127.5 - 1.0;
        }
        v.noise
    } else if ctrl & 0x40 != 0 {
        if p < pw { 1.0 } else { -1.0 }
    } else if ctrl & 0x20 != 0 {
        (2.0 * p - 1.0) as f32
    } else if ctrl & 0x10 != 0 {
        (1.0 - 4.0 * (p - 0.5).abs()) as f32
    } else {
        0.0
    };
    wave * v.env as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_song_the_game_cues_is_in_the_data() {
        for s in [
            Song::Accid,
            Song::Heroic,
            Song::Danger,
            Song::Sword,
            Song::Rejoin,
            Song::Shadow,
            Song::Vict,
            Song::Stairs,
            Song::Upstairs,
            Song::Potion,
            Song::ShortPot,
            Song::Timer,
            Song::Tragic,
            Song::Embrace,
            Song::Heartbeat,
            Song::Princess,
            Song::Vizier,
            Song::Buildup,
            Song::Magic,
        ] {
            assert!(records(s.c64()).is_some(), "{s:?}");
            let t = seconds(s);
            assert!((0.5..30.0).contains(&t), "{s:?} lasts {t} s");
        }
    }

    #[test]
    fn heartbeat_is_a_low_pulse() {
        // a 98 Hz saw, gated on and off a little more than once a second
        let v = render(Song::Heartbeat, 8000);
        assert!(!v.is_empty());
        let loud: Vec<bool> = v.chunks(400).map(|c| c.iter().map(|x| x.abs()).fold(0.0, f32::max) > 0.02).collect();
        let beats = loud.windows(2).filter(|w| !w[0] && w[1]).count();
        let secs = v.len() as f32 / 8000.0;
        assert!((0.6..2.0).contains(&(beats as f32 / secs)), "{beats} beats in {secs} s");
    }
}

#[cfg(test)]
mod wav {
    use super::*;

    /// `POB_WAV_DIR=dir cargo test write_wavs -- --ignored` writes every
    /// song as a WAV file, for listening.
    #[test]
    #[ignore]
    fn write_wavs() {
        let dir = std::env::var("POB_WAV_DIR").expect("POB_WAV_DIR");
        for (name, s) in [
            ("accid", Song::Accid),
            ("heroic", Song::Heroic),
            ("danger", Song::Danger),
            ("sword", Song::Sword),
            ("rejoin", Song::Rejoin),
            ("shadow", Song::Shadow),
            ("vict", Song::Vict),
            ("stairs", Song::Stairs),
            ("upstairs", Song::Upstairs),
            ("potion", Song::Potion),
            ("shortpot", Song::ShortPot),
            ("timer", Song::Timer),
            ("tragic", Song::Tragic),
            ("embrace", Song::Embrace),
            ("heartbeat", Song::Heartbeat),
            ("vizier", Song::Vizier),
            ("buildup", Song::Buildup),
            ("magic", Song::Magic),
        ] {
            let v = render(s, 44_100);
            let mut out = Vec::new();
            let data = (v.len() * 2) as u32;
            out.extend_from_slice(b"RIFF");
            out.extend_from_slice(&(36 + data).to_le_bytes());
            out.extend_from_slice(b"WAVEfmt ");
            out.extend_from_slice(&[16, 0, 0, 0, 1, 0, 1, 0]);
            out.extend_from_slice(&44_100u32.to_le_bytes());
            out.extend_from_slice(&88_200u32.to_le_bytes());
            out.extend_from_slice(&[2, 0, 16, 0]);
            out.extend_from_slice(b"data");
            out.extend_from_slice(&data.to_le_bytes());
            for x in v {
                out.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
            }
            std::fs::write(format!("{dir}/{name}.wav"), out).unwrap();
        }
    }
}
