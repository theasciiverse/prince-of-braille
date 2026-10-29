//! The princess's room: the pre-recorded scenes (SUBS.S PLAYCUT and
//! PlayCut0-8, the start positions startP0 ... startK7, the playback loop
//! `play`) and the room's animations (pburn, pstars, pflow).
//!
//! A song step starts the song and plays on for the number of frames the
//! original gave it with the music off.

use crate::game::*;
use crate::seq::seq;
use crate::music::Song as S;

/// GAMEEQ floorY in the princess's room
pub const FLOOR_Y: u8 = 151;
/// SUBS.S ptorchstate: the two torches' starting flame frames
const PTORCHSTATE: [u8; 2] = [1, 6];

/// What follows a scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum After {
    /// start this level (LoadNextLevel)
    Level(u8),
    /// the scene in the title's story; the title goes on after it
    Attract,
    TimeUp,
    Won,
}

#[derive(Clone, Copy, Debug)]
enum Step {
    /// `play`: run this many frames
    Play(u32),
    /// PlaySongX/PlaySongI/PlaySong: start the song, play on this many frames
    Song(S, u32),
    /// SPEED
    Speed(u8),
    /// pjumpseq: the princess
    P(u8),
    /// kjumpseq/vjumpseq/mjumpseq: the kid, vizier or mouse
    K(u8),
    /// KidPosn := 0, he disappears
    KidGone,
    /// addglass: the hourglass in state X, sand flowing
    Glass(u8),
    /// addglass1: the hourglass in state X
    Glass1(u8),
    /// psandcount := 0, the sand starts flowing
    Sand,
    /// a white lightning flash
    Flash,
    StartP(Start),
    StartK(Start),
}

#[derive(Clone, Copy, Debug)]
enum Start {
    P0,
    P1,
    P2,
    P4,
    P5,
    P7,
    P8,
    V0,
    K7,
    M4,
    M7,
    M8,
}

/// A scene being played.
#[derive(Clone, Debug)]
pub struct Cut {
    pub num: u8,
    /// the level being entered: it decides which disk side's chtable6 is used
    pub level: u8,
    pub after: After,
    steps: Vec<Step>,
    pc: usize,
    /// SceneCount: frames left in the current `play`
    count: u32,
    pub speed: u8,
    /// GlassState (0 full - 8 empty), None before the hourglass appears
    pub glass: Option<u8>,
    /// psandcount: frame of the falling sand, -1 before it flows
    pub sand: i8,
    /// ptorchstate
    pub torch: [u8; 2],
    /// pstarcount: frames each star has left to twinkle
    pub stars: [u8; 4],
    /// real time owed to the next frame, in ms
    acc: f32,
    btn_was: bool,
}

/// One frame of the playback loop at SPEED `s`, in ms. PAUSE spends about
/// 1.26 ms per unit of SPEED (256 turns of a 5-cycle loop at 1.023 MHz);
/// the rest is an estimate of drawing the frame, chosen so that the usual
/// SPEED 12 gives the game's own 12 frames a second.
fn frame_ms(s: u8) -> f32 {
    1000.0 / FPS as f32 - 1.26 * 12.0 + 1.26 * s as f32
}

impl Game {
    /// PLAYCUT: start scene `num` in the princess's room, then `after`.
    pub fn playcut(&mut self, num: u8, level: u8, after: After) {
        use Step::*;
        let glass = self.getglass();
        let steps: Vec<Step> = match num {
            0 => vec![
                StartK(Start::V0),
                StartP(Start::P0),
                Play(2),
                Song(S::Princess, 8),
                Play(5),
                P(seq::PALERT),
                Play(9),
                Speed(7),
                Play(5),
                K(seq::VAPPROACH),
                Play(6),
                K(seq::VSTOP),
                Play(4),
                Song(S::Jaffar, 12),
                Play(4),
                K(seq::VAPPROACH),
                Play(30),
                K(seq::VSTOP),
                Play(4),
                Play(25),
                K(seq::VRAISE),
                Play(1),
                P(seq::PBACK),
                Play(13),
                Glass1(0),
                Flash,
                Speed(12),
                Play(5),
                Sand,
                Play(8),
                Speed(7),
                K(seq::VEXIT),
                Play(17),
                Glass1(1),
                Play(12),
                P(seq::PSLUMP),
                Play(28),
                Speed(12),
                Song(S::Timer, 20),
            ],
            1 | 3 => vec![Glass(glass), StartP(Start::P1), Play(2), Song(S::Timer, 50)],
            2 => vec![Glass(glass), StartP(Start::P2), Play(2), Song(S::Heartbeat, 50)],
            4 => vec![
                Glass(glass),
                StartP(Start::P4),
                StartK(Start::M4),
                Play(5),
                P(seq::PCROUCH),
                Play(9),
                K(seq::MRAISE),
                Play(58),
            ],
            5 if glass >= 7 => vec![
                // Ominous: the sand is almost out
                Glass(glass),
                StartP(Start::P5),
                Play(2),
                Song(S::Heartbeat, 50),
                P(seq::PALERT),
                Play(12),
                Song(S::Danger, 20),
            ],
            5 => vec![Glass(glass), StartP(Start::P1), Play(2), Song(S::Timer, 50)],
            6 => vec![Speed(22), Glass1(8), Play(2), Song(S::Tragic, 0), Play(100)],
            7 => vec![
                Speed(8),
                StartP(Start::P7),
                Play(8),
                StartK(Start::K7),
                Play(8),
                P(seq::PEMBRACE),
                Play(5),
                K(seq::RUNSTOP),
                Play(2),
                KidGone,
                Play(9),
                Song(S::Embrace, 0),
                StartK(Start::M7),
                Play(12),
                K(seq::MCLIMB),
                Play(30),
            ],
            8 => vec![
                Glass(glass),
                StartP(Start::P8),
                StartK(Start::M8),
                Play(20),
                K(seq::MLEAVE),
                Play(20),
                P(seq::PRISE),
                Play(20),
                KidGone,
                Song(S::Heartbeat, 50),
            ],
            _ => vec![],
        };
        log::info!("princess cut {num}, then {after:?}");
        // initit
        self.kid = Char { posn: 0, ..Char::default() };
        self.shad = Char { posn: 0, face: crate::guard::NO_CHAR, ..Char::default() };
        self.message = None;
        self.lightning = 0;
        self.cut = Some(Cut {
            num,
            level,
            after,
            steps,
            pc: 0,
            count: 0,
            speed: 12,
            glass: None,
            sand: -1,
            torch: PTORCHSTATE,
            stars: [0; 4],
            acc: 0.0,
            btn_was: true,
        });
        self.phase = Phase::Cut;
    }

    /// getglass: the hourglass's state from the time left.
    fn getglass(&self) -> u8 {
        match self.time_left().0 {
            m if m < 6 => 7,
            m if m < 11 => 6,
            m if m < 21 => 5,
            m if m < 41 => 4,
            _ => 3,
        }
    }

    /// One game tick during a scene: play as many frames as the scene's
    /// speed allows. The button skips the rest of the scene.
    pub fn cut_tick(&mut self, c: Controls) {
        let Some(mut cut) = self.cut.take() else { return };
        let fresh = c.btn && !cut.btn_was;
        cut.btn_was = c.btn;
        if fresh {
            log::info!("princess cut {} skipped", cut.num);
            self.end_cut(cut.after);
            return;
        }
        cut.acc += 1000.0 / FPS as f32;
        loop {
            // run the script up to the next frame to play
            while cut.count == 0 {
                let Some(&step) = cut.steps.get(cut.pc) else {
                    self.end_cut(cut.after);
                    return;
                };
                cut.pc += 1;
                self.cut_step(&mut cut, step);
            }
            let ms = frame_ms(cut.speed);
            if cut.acc < ms {
                break;
            }
            cut.acc -= ms;
            cut.count -= 1;
            self.cut_frame(&mut cut);
        }
        self.cut = Some(cut);
    }

    fn cut_step(&mut self, cut: &mut Cut, step: Step) {
        match step {
            Step::Play(n) => cut.count = n,
            Step::Song(song, n) => {
                self.cuesong(song);
                cut.count = n;
            }
            Step::Speed(s) => cut.speed = s,
            Step::P(n) => {
                self.load_shad();
                self.jumpseq(n);
                self.save_shad();
            }
            Step::K(n) => {
                self.load_kid();
                self.jumpseq(n);
                self.save_kid();
            }
            Step::KidGone => self.kid.posn = 0,
            Step::Glass(x) => {
                cut.sand = 0;
                cut.glass = Some(x);
            }
            Step::Glass1(x) => cut.glass = Some(x),
            Step::Sand => cut.sand = 0,
            Step::Flash => {
                self.lightning = 5;
                self.lightcolor = 0xff;
            }
            Step::StartP(s) => {
                self.ch = Char { face: crate::guard::NO_CHAR, ..Char::default() };
                self.cut_start(s);
                self.save_shad();
            }
            Step::StartK(s) => {
                self.ch = Char::default();
                self.cut_start(s);
                self.save_kid();
            }
        }
    }

    /// The start positions (SUBS.S startP0 ... startK7), on Char. They
    /// build on each other as in the source, so some run animchar twice.
    fn cut_start(&mut self, s: Start) {
        let place = |g: &mut Game, x: u8, y: u8, sq: u8| {
            g.ch.x = x;
            g.ch.y = y;
            g.jumpseq(sq);
            g.animchar();
        };
        match s {
            Start::P0 => {
                self.ch.id = 5;
                self.ch.face = FACE_LEFT;
                self.ch.life = 0xff;
                self.ch.scrn = 1;
                place(self, 120, FLOOR_Y, seq::PSTAND);
            }
            Start::P1 => {
                self.cut_start(Start::P0);
                self.ch.face = FACE_RIGHT;
            }
            Start::P2 => {
                self.cut_start(Start::P0);
                place(self, 89, FLOOR_Y, seq::PLIE);
            }
            Start::P4 => {
                self.cut_start(Start::P1);
                place(self, 142, FLOOR_Y + 3, seq::PSTAND);
            }
            Start::P5 => {
                self.cut_start(Start::P0);
                self.ch.x = 160;
            }
            Start::P7 => {
                self.cut_start(Start::P0);
                place(self, 136, FLOOR_Y - 2, seq::PWAITING);
            }
            Start::P8 => {
                self.cut_start(Start::P0);
                place(self, 130, FLOOR_Y + 3, seq::PSTROKE);
            }
            Start::V0 => {
                self.ch.id = 6;
                self.ch.face = FACE_LEFT;
                self.ch.life = 0xff;
                self.ch.scrn = 1;
                place(self, 197, FLOOR_Y, seq::VSTAND);
            }
            Start::K7 => {
                self.ch.id = 0;
                self.ch.face = FACE_LEFT;
                self.ch.life = 0xff;
                self.ch.scrn = 1;
                place(self, 198, FLOOR_Y - 2, seq::STARTRUN);
            }
            Start::M4 => {
                self.ch.id = 24;
                self.ch.face = FACE_LEFT;
                self.ch.life = 0xff;
                self.ch.scrn = 1;
                place(self, 199, FLOOR_Y + 1, seq::MSCURRY);
            }
            Start::M7 => {
                self.cut_start(Start::M4);
                self.ch.y = FLOOR_Y - 2;
            }
            Start::M8 => {
                self.cut_start(Start::M4);
                place(self, 144, FLOOR_Y + 1, seq::MSTOP);
            }
        }
    }

    /// One frame of `play`: NextFrame (DoKid, DoShad), the flash, and the
    /// room's own animation (pburn twice, pstars, pflow).
    fn cut_frame(&mut self, cut: &mut Cut) {
        self.rnd();
        if self.kid.posn != 0 {
            self.load_kid();
            self.animchar();
            self.save_kid();
        }
        if self.shad.posn != 0 {
            self.load_shad_wop();
            self.animchar();
            self.save_shad_wop();
        }
        if self.lightning > 0 {
            self.lightning -= 1;
        }
        for t in 0..2 {
            cut.torch[t] = self.flameframe(cut.torch[t]);
        }
        // pstars: a running twinkle counts down; maybe start a new one
        for s in cut.stars.iter_mut() {
            *s = s.saturating_sub(1);
        }
        if self.rnd() < 10 {
            let len = (self.rnd() & 3) + 5;
            self.rnd();
            let n = (self.rnd() & 3) as usize;
            cut.stars[n] = len;
        }
        // pflow
        if cut.sand >= 0 {
            cut.sand = if cut.sand >= 2 { 0 } else { cut.sand + 1 };
        }
    }

    fn end_cut(&mut self, after: After) {
        self.cut = None;
        self.lightning = 0;
        log::info!("princess cut over: {after:?}");
        match after {
            After::Level(n) => {
                self.start_level(n, true);
                if n != 14 {
                    self.time_request = 1;
                }
            }
            After::Attract => self.demo_over = true,
            After::TimeUp => {
                self.phase = Phase::TimeUp;
                self.show_message("TIME HAS EXPIRED".into(), u32::MAX);
            }
            After::Won => self.phase = Phase::Won,
        }
    }
}

/// The scene played before entering level `n` (TOPCTRL.S LoadNextLevel).
pub fn cut_before(n: u8) -> Option<u8> {
    match n {
        2 => Some(1),
        4 => Some(2),
        6 => Some(3),
        8 => Some(8),
        9 => Some(4),
        12 => Some(5),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Game, ticks: u32) {
        for _ in 0..ticks {
            g.tick(Controls::default());
        }
    }

    #[test]
    fn climbing_out_of_level_1_plays_cut_1_then_starts_level_2() {
        let mut g = Game::new(1);
        g.phase = Phase::LevelDone(0);
        run(&mut g, 2);
        assert_eq!(g.phase, Phase::Cut);
        assert_eq!(g.cut.as_ref().map(|c| c.num), Some(1));
        assert_eq!(g.shad.id, 5, "the princess is in the room");
        run(&mut g, 200);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.level.number, 2);
    }

    #[test]
    fn the_button_skips_a_scene() {
        let mut g = Game::new(3);
        g.phase = Phase::LevelDone(0);
        run(&mut g, 2);
        assert_eq!(g.cut.as_ref().map(|c| c.num), Some(2));
        g.tick(Controls { btn: true, ..Controls::default() });
        assert_eq!(g.level.number, 4);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn the_opening_scene_turns_the_hourglass() {
        let mut g = Game::new(1);
        g.playcut(0, 1, After::Attract);
        let mut saw_vizier = false;
        let mut saw_glass = false;
        for _ in 0..400 {
            g.tick(Controls::default());
            if let Some(c) = &g.cut {
                saw_vizier |= g.kid.id == 6 && g.kid.posn != 0;
                saw_glass |= c.glass.is_some();
            }
        }
        assert!(saw_vizier && saw_glass);
        assert!(g.cut.is_none() && g.demo_over);
    }

    #[test]
    fn the_hourglass_runs_down_with_the_clock() {
        let mut g = Game::new(5);
        assert_eq!(g.getglass(), 3);
        g.frame_count = 50 * MIN_FRAMES;
        assert_eq!(g.getglass(), 6);
        g.frame_count = 57 * MIN_FRAMES;
        assert_eq!(g.getglass(), 7);
    }
}
