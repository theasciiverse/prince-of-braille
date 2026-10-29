//! Keyboard and gamepad to "joystick".
//!
//! Prince of Persia needs held keys: running is "forward held", grabbing a
//! ledge is "button held". Two tiers:
//!
//! 1. **Exact**: the terminal speaks the kitty keyboard protocol, so key
//!    releases are real and several keys can be held at once.
//! 2. **Timing**: other terminals only send presses and OS key repeat. A key
//!    counts as held while its repeats keep arriving. Before the first
//!    repeat a tap and a hold look the same, so a lone press holds for
//!    about one repeat delay. This is close to the Apple II itself, whose
//!    keyboard also only knew "the last key, and is it still down".
//!
//! A gamepad (the `gamepad` feature, on by default) is always exact. It is
//! mapped as SDLPoP maps one: the d-pad or either stick moves, Y is up, A
//! is down, X or a trigger is the button (Shift), and Start or Back pause.
//! Here Start pauses and Back asks to quit, as Esc does, and the
//! shoulder buttons are the button too, for pads without triggers.

use crate::game::Controls;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode};
use std::time::{Duration, Instant};

/// How long a single press counts as held before repeats prove it.
const PRE_REPEAT: Duration = Duration::from_millis(550);
/// Gap between repeats after which the key counts as released.
const RELEASE: Duration = Duration::from_millis(140);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Exact,
    Timing,
}

#[derive(Clone, Copy, Default, Debug)]
struct Hold {
    down: bool,
    last: Option<Instant>,
    count: u32,
}

impl Hold {
    fn press(&mut self, now: Instant, mode: Mode) {
        if mode == Mode::Exact {
            self.down = true;
            self.last = Some(now);
            return;
        }
        match self.last {
            Some(l) if self.count >= 1 && now.duration_since(l) <= PRE_REPEAT + RELEASE => self.count += 1,
            _ => self.count = 1,
        }
        self.last = Some(now);
    }

    fn release(&mut self) {
        self.down = false;
        self.last = None;
        self.count = 0;
    }

    fn held(&self, now: Instant, mode: Mode) -> bool {
        if mode == Mode::Exact {
            return self.down;
        }
        let Some(l) = self.last else { return false };
        let t = if self.count >= 2 { RELEASE } else { PRE_REPEAT };
        now.duration_since(l) <= t
    }
}

/// Non-movement commands.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    Quit,
    AskQuit,
    Continue,
    RestartLevel,
    NextLevel,
    PrevLevel,
    Glyphs,
    /// the DOS art or the Apple picture
    View,
    Scale,
    Pause,
    Theme,
    Mute,
    /// Ctrl-G
    SaveGame,
    /// Ctrl-L
    LoadGame,
}

/// A gamepad press outside the game's controls: main decides what it does
/// (at the title any of them plays).
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
pub enum PadPress {
    /// a face button, a trigger or a shoulder
    Face,
    Start,
    Back,
}

/// SDLPoP's joystick_threshold, 8000 of 32767.
#[cfg(feature = "gamepad")]
const STICK: f32 = 8000.0 / 32767.0;

/// Only pads with an SDL mapping: gilrs also lists things like
/// accelerometers and keyboards as gamepads (POB_ANY_PAD=1 takes any).
#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("POB_ANY_PAD").is_ok_and(|v| v == "1")
}

/// SDLPoP get_joystick_state: a stick's direction from its angle, in
/// overlapping sectors so the diagonals give both. `keep_x` and `keep_y`
/// are SDLPoP's two tweaks: pushing straight up keeps a running jump
/// running, and easing off a crouch's landing doesn't stand him up.
#[cfg(feature = "gamepad")]
fn stick(x: f32, y: f32, prev: [i8; 2], running_jump: bool, crouch_landing: bool) -> [i8; 2] {
    if x * x + y * y < STICK * STICK {
        return [0, 0];
    }
    // degrees: 0 right, 90 up (gilrs' y points up, SDL's down)
    let a = y.atan2(x).to_degrees();
    let mut out = prev;
    out[0] = if a.abs() < 60.0 {
        1
    } else if a.abs() > 120.0 {
        -1
    } else if a > 0.0 && running_jump {
        prev[0]
    } else {
        0
    };
    out[1] = if a > 30.0 && a < 150.0 {
        -1
    } else if a < -35.0 && a > -145.0 {
        1
    } else if a < 0.0 && crouch_landing {
        prev[1]
    } else {
        0
    };
    out
}

pub struct Input {
    pub mode: Mode,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    /// the gamepad in use, if any
    pub pad_name: Option<String>,
    /// the pad's controls: x, y, button
    pad: (i8, i8, bool),
    /// the two sticks' directions, kept between frames for SDLPoP's tweaks
    #[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
    sticks: [[i8; 2]; 2],
    pub pad_presses: Vec<PadPress>,
    left: Hold,
    right: Hold,
    up: Hold,
    down: Hold,
    btn: Hold,
    /// which horizontal key was pressed last (wins if both held)
    last_x: i8,
    last_y: i8,
    pub cmds: Vec<Cmd>,
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input {
            mode: if exact { Mode::Exact } else { Mode::Timing },
            left: Hold::default(),
            right: Hold::default(),
            up: Hold::default(),
            down: Hold::default(),
            btn: Hold::default(),
            last_x: 0,
            last_y: 0,
            cmds: Vec::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad_name: None,
            pad: (0, 0, false),
            sticks: [[0; 2]; 2],
            pad_presses: Vec::new(),
        }
    }

    pub fn open_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(_, p)| (p.name().to_string(), usable_pad(&p))).collect();
                log::info!("gamepads (name, used): {all:?}");
                self.pad_name = all.into_iter().find(|(_, used)| *used).map(|(n, _)| n);
                self.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
    }

    /// Drain the gamepad's events and read its controls, once a frame.
    /// `kid` is the kid's action and frame, for SDLPoP's stick tweaks.
    #[cfg_attr(not(feature = "gamepad"), allow(unused_variables))]
    pub fn poll_pad(&mut self, kid: (u8, u8)) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Axis, Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            let mut used = None;
            while let Some(ev) = g.next_event() {
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => {
                        self.pad_name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.pad_name);
                    }
                    EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        self.pad_name = None;
                    }
                    EventType::ButtonPressed(b, _) => {
                        log::debug!("pad press {b:?}");
                        let p = match b {
                            Button::Start => Some(PadPress::Start),
                            Button::Select => Some(PadPress::Back),
                            Button::South
                            | Button::East
                            | Button::North
                            | Button::West
                            | Button::LeftTrigger
                            | Button::RightTrigger
                            | Button::LeftTrigger2
                            | Button::RightTrigger2 => Some(PadPress::Face),
                            _ => None,
                        };
                        self.pad_presses.extend(p);
                    }
                    EventType::ButtonReleased(b, _) => log::debug!("pad release {b:?}"),
                    _ => {}
                }
                used = Some(ev.id);
            }
            // the pad last heard from, or the first usable one
            let id = used.or_else(|| g.gamepads().find(|(_, p)| usable_pad(p)).map(|(id, _)| id));
            let Some(pad) = id.map(|id| g.gamepad(id)) else {
                self.pad = (0, 0, false);
                return;
            };
            let on = |b: Button| pad.is_pressed(b);
            let axis = |a: Axis| pad.axis_data(a).map_or(0.0, |d| d.value());
            let trigger = |b: Button| pad.button_data(b).is_some_and(|d| d.value() > STICK);
            let (action, posn) = kid;
            let running_jump = action == 1;
            let crouch_landing = (108..=112).contains(&posn);
            self.sticks[0] =
                stick(axis(Axis::LeftStickX), axis(Axis::LeftStickY), self.sticks[0], running_jump, crouch_landing);
            self.sticks[1] =
                stick(axis(Axis::RightStickX), axis(Axis::RightStickY), self.sticks[1], running_jump, crouch_landing);
            let [l, r] = self.sticks;
            let left = l[0] < 0 || r[0] < 0 || on(Button::DPadLeft);
            let right = l[0] > 0 || r[0] > 0 || on(Button::DPadRight);
            let up = l[1] < 0 || r[1] < 0 || on(Button::DPadUp) || on(Button::North);
            let down = l[1] > 0 || r[1] > 0 || on(Button::DPadDown) || on(Button::South);
            let btn = on(Button::West)
                || on(Button::LeftTrigger)
                || on(Button::RightTrigger)
                || trigger(Button::LeftTrigger2)
                || trigger(Button::RightTrigger2);
            // SDLPoP: left wins over right, up over down
            let x = if left { -1 } else if right { 1 } else { 0 };
            let y = if up { -1 } else if down { 1 } else { 0 };
            self.pad = (x, y, btn);
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind != KeyEventKind::Release;
        let fresh = k.kind == KeyEventKind::Press;
        if k.modifiers.contains(KeyModifiers::CONTROL) {
            match k.code {
                KeyCode::Char('c') => self.cmds.push(Cmd::Quit),
                KeyCode::Char('g') if fresh => self.cmds.push(Cmd::SaveGame),
                KeyCode::Char('l') if fresh => self.cmds.push(Cmd::LoadGame),
                _ => {}
            }
            if matches!(k.code, KeyCode::Char('c' | 'g' | 'l')) {
                return;
            }
        }
        let mode = self.mode;
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        let dir = |h: &mut Hold| if press { h.press(now, mode) } else { h.release() };
        let code = match k.code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            c => c,
        };
        match code {
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') | KeyCode::Char('4') => {
                dir(&mut self.left);
                if press {
                    self.last_x = -1;
                }
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') | KeyCode::Char('6') => {
                dir(&mut self.right);
                if press {
                    self.last_x = 1;
                }
            }
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') | KeyCode::Char('8') => {
                dir(&mut self.up);
                if press {
                    self.last_y = -1;
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') | KeyCode::Char('2') | KeyCode::Char('5') => {
                dir(&mut self.down);
                if press {
                    self.last_y = 1;
                }
            }
            // diagonal jumps, like the Apple's U/O keys and the DOS keypad
            KeyCode::Home | KeyCode::Char('q') | KeyCode::Char('7') | KeyCode::Char('y') => {
                dir(&mut self.up);
                dir(&mut self.left);
                if press {
                    self.last_x = -1;
                    self.last_y = -1;
                }
            }
            KeyCode::PageUp | KeyCode::Char('e') | KeyCode::Char('9') | KeyCode::Char('u') => {
                dir(&mut self.up);
                dir(&mut self.right);
                if press {
                    self.last_x = 1;
                    self.last_y = -1;
                }
            }
            KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => dir(&mut self.btn),
            KeyCode::Char(' ') | KeyCode::Char('z') => {
                dir(&mut self.btn);
                if fresh {
                    self.cmds.push(Cmd::Continue);
                }
            }
            KeyCode::Enter if fresh => self.cmds.push(Cmd::Continue),
            KeyCode::Esc if fresh => self.cmds.push(Cmd::AskQuit),
            KeyCode::Char('r') if fresh => self.cmds.push(Cmd::RestartLevel),
            KeyCode::Char(']') | KeyCode::Char('n') if fresh => self.cmds.push(Cmd::NextLevel),
            KeyCode::Char('[') | KeyCode::Char('b') if fresh => self.cmds.push(Cmd::PrevLevel),
            KeyCode::Char('g') if fresh => self.cmds.push(Cmd::Glyphs),
            KeyCode::Char('v') if fresh => self.cmds.push(Cmd::View),
            KeyCode::Char('f') if fresh => self.cmds.push(Cmd::Scale),
            KeyCode::Char('p') if fresh => self.cmds.push(Cmd::Pause),
            KeyCode::Char('c') if fresh => self.cmds.push(Cmd::Theme),
            KeyCode::Char('m') | KeyCode::F(8) if fresh => self.cmds.push(Cmd::Mute),
            _ => {}
        }
        // timing mode: Shift on an arrow key means the button is down too
        if self.mode == Mode::Timing && shift && press {
            self.btn.press(now, mode);
        }
    }

    pub fn controls(&self, now: Instant) -> Controls {
        let m = self.mode;
        let (l, r) = (self.left.held(now, m), self.right.held(now, m));
        let x = match (l, r) {
            (true, false) => -1,
            (false, true) => 1,
            (true, true) => self.last_x,
            _ => 0,
        };
        let (u, d) = (self.up.held(now, m), self.down.held(now, m));
        let y = match (u, d) {
            (true, false) => -1,
            (false, true) => 1,
            (true, true) => self.last_y,
            _ => 0,
        };
        let (px, py, pb) = self.pad;
        Controls {
            x: if px != 0 { px } else { x },
            y: if py != 0 { py } else { y },
            btn: pb || self.btn.held(now, m),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(i: &mut Input, c: KeyCode, t: Instant) {
        i.on_key(KeyEvent::new(c, KeyModifiers::NONE), t);
    }

    #[test]
    fn a_tap_holds_about_one_repeat_delay() {
        let t0 = Instant::now();
        let mut i = Input::new(false);
        key(&mut i, KeyCode::Right, t0);
        assert_eq!(i.controls(t0 + Duration::from_millis(300)).x, 1);
        assert_eq!(i.controls(t0 + Duration::from_millis(700)).x, 0);
    }

    #[test]
    fn repeats_keep_it_held_and_release_is_quick() {
        let t0 = Instant::now();
        let mut i = Input::new(false);
        key(&mut i, KeyCode::Left, t0);
        for k in 0..20 {
            key(&mut i, KeyCode::Left, t0 + Duration::from_millis(500 + 33 * k));
        }
        let last = t0 + Duration::from_millis(500 + 33 * 19);
        assert_eq!(i.controls(last + Duration::from_millis(100)).x, -1);
        assert_eq!(i.controls(last + Duration::from_millis(200)).x, 0);
    }

    #[test]
    fn exact_mode_uses_releases() {
        let t0 = Instant::now();
        let mut i = Input::new(true);
        i.on_key(KeyEvent::new_with_kind(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Press), t0);
        assert_eq!(i.controls(t0 + Duration::from_secs(5)).y, -1);
        i.on_key(KeyEvent::new_with_kind(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Release), t0);
        assert_eq!(i.controls(t0).y, 0);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn the_stick_reads_as_sdlpop_reads_it() {
        let d = std::f32::consts::PI / 180.0;
        let at = |deg: f32| (deg * d).cos();
        let at_y = |deg: f32| (deg * d).sin();
        let s = |deg: f32, prev: [i8; 2], rj: bool| stick(at(deg), at_y(deg), prev, rj, false);
        assert_eq!(s(0.0, [0, 0], false), [1, 0]);
        assert_eq!(s(90.0, [0, 0], false), [0, -1]);
        // up-left is a diagonal jump
        assert_eq!(s(135.0, [0, 0], false), [-1, -1]);
        // down's sector is narrower: 30 below level is still just right
        assert_eq!(s(-32.0, [0, 0], false), [1, 0]);
        // straight up in a running jump keeps him running
        assert_eq!(s(90.0, [1, 0], true), [1, -1]);
        assert_eq!(s(90.0, [1, 0], false), [0, -1]);
        // the dead zone
        assert_eq!(stick(0.1, 0.1, [1, 1], false, false), [0, 0]);
    }
}
