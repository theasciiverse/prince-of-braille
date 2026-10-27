//! Keyboard to "joystick".
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
    Scale,
    Pause,
    Theme,
    Mute,
}

pub struct Input {
    pub mode: Mode,
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
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind != KeyEventKind::Release;
        let fresh = k.kind == KeyEventKind::Press;
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.cmds.push(Cmd::Quit);
            return;
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
        Controls { x, y, btn: self.btn.held(now, m) }
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
}
