//! Prince of Braille: Jordan Mechner's Apple II Prince of Persia (1989),
//! restored for the terminal from his released 6502 source.
//!
//! The game logic is a port of the original routines (CTRL.S, COLL.S,
//! MOVER.S, FRAMEADV.S, ...), driven by the original sequence and frame
//! tables and drawn from the original image tables and level blueprints.
//! The 280 x 192 hi-res screen is rebuilt pixel for pixel, then shown with
//! Braille for the prince and block glyphs for the dungeon.
//!
//! Session log: `RUST_LOG=debug prince-of-braille 2>pob.log`

mod apple;
mod bg;
mod coll;
mod ctrl;
mod game;
mod guard;
mod input;
mod level;
mod mover;
mod render;
mod seq;
mod subs;
mod theme;

use apple::Screen;
use game::{Game, Phase, FPS};
use input::{Cmd, Input};
use ratatui::crossterm::event::{self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::DefaultTerminal;
use render::Glyphs;
use std::io;
use std::time::{Duration, Instant};

struct App {
    game: Game,
    screen: Screen,
    glyphs: Glyphs,
    theme: theme::Theme,
    /// 0 = fit automatically, else forced 1 or 2
    scale: i32,
    paused: bool,
    confirm_quit: bool,
    quit: bool,
}

fn usage() -> ! {
    eprintln!("usage: prince-of-braille [--theme dusk|phosphor|amber|classic] [LEVEL]   (level 1-14, default 1)");
    std::process::exit(2);
}

fn main() -> io::Result<()> {
    env_logger::init();
    let mut level = 1u8;
    let mut dump: Option<String> = None;
    let mut theme = theme::load().unwrap_or(theme::Theme::Dusk);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => usage(),
            "--dump" => dump = Some(args.next().unwrap_or_default()),
            "--theme" => {
                theme = args.next().as_deref().and_then(theme::Theme::from_name).unwrap_or_else(|| usage());
            }
            n => level = n.parse().ok().filter(|v| (1..=14).contains(v)).unwrap_or_else(|| usage()),
        }
    }
    if let Some(script) = dump {
        dump_frames(level, &script, theme);
        return Ok(());
    }
    let mut terminal = ratatui::init();
    let exact = supports_keyboard_enhancement().unwrap_or(false);
    if exact {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    log::info!("start level={level} exact_keys={exact}");
    let result = run(&mut terminal, level, exact, theme);
    if exact {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, level: u8, exact: bool, theme: theme::Theme) -> io::Result<()> {
    let mut app = App {
        game: Game::new(level),
        screen: Screen::default(),
        glyphs: Glyphs::Hybrid,
        theme,
        scale: 0,
        paused: false,
        confirm_quit: false,
        quit: false,
    };
    let mut input = Input::new(exact);
    let tick = Duration::from_secs(1) / FPS;
    let mut next = Instant::now() + tick;
    let mut last_snapshot = Instant::now();
    while !app.quit {
        terminal.draw(|f| draw(f, &mut app, &input))?;
        while event::poll(next.saturating_duration_since(Instant::now()))? {
            if let Event::Key(k) = event::read()? {
                log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                input.on_key(k, Instant::now());
            }
        }
        for cmd in std::mem::take(&mut input.cmds) {
            handle(&mut app, cmd);
        }
        let now = Instant::now();
        let mut steps = 0;
        while now >= next && steps < 3 {
            if !app.paused && !app.confirm_quit {
                let c = input.controls(now);
                app.game.tick(c);
            }
            next += tick;
            steps += 1;
        }
        if now >= next {
            next = now + tick;
        }
        if now.duration_since(last_snapshot) >= Duration::from_millis(500) {
            last_snapshot = now;
            let g = &app.game;
            let k = &g.kid;
            log::debug!(
                "state level={} scrn={} x={} y={} posn={} action={} face={} bx={} by={} str={}/{} phase={:?} ctl={:?}",
                g.level.number,
                k.scrn,
                k.x,
                k.y,
                k.posn,
                k.action,
                if k.face == game::FACE_LEFT { "L" } else { "R" },
                k.block_x as i8,
                k.block_y as i8,
                g.kid_strength,
                g.max_strength,
                g.phase,
                input.controls(now)
            );
            if g.shad.face != guard::NO_CHAR {
                let o = &g.shad;
                log::debug!(
                    "opp id={} scrn={} x={} y={} posn={} action={} sword={} life={:#x} str={}/{} alert={} prog={}",
                    o.id, o.scrn, o.x, o.y, o.posn, o.action, o.sword, o.life, g.opp_strength, g.max_opp_str,
                    g.enemy_alert, g.guardprog
                );
            }
        }
    }
    Ok(())
}

/// Headless run for testing: `--dump "R:12 .:6 U:1 =:3"` holds the given
/// keys (L R U D B, `.` none) for N frames each and prints the screen as
/// text after every token. A token starting with `=` just prints.
fn dump_frames(level: u8, script: &str, theme: theme::Theme) {
    let mut g = Game::new(level);
    let mut scr = Screen::default();
    let (w, h) = render::cells_needed(1);
    for tok in script.split_whitespace() {
        let (keys, n) = tok.split_once(':').unwrap_or((tok, "1"));
        let n: u32 = n.parse().unwrap_or(1);
        let mut c = game::Controls::default();
        for ch in keys.chars() {
            match ch {
                'L' => c.x = -1,
                'R' => c.x = 1,
                'U' => c.y = -1,
                'D' => c.y = 1,
                'B' => c.btn = true,
                _ => {}
            }
        }
        if !keys.starts_with('=') {
            for _ in 0..n {
                g.tick(c);
            }
        }
        bg::draw(&mut scr, &g);
        if let Ok(dir) = std::env::var("POB_PPM_DIR") {
            // the monitor's view, for checking against the original
            let mut out = format!("P6 {} {} 255\n", apple::SCREEN_W, apple::SCREEN_H).into_bytes();
            for c in scr.colors() {
                out.extend_from_slice(&c);
            }
            let _ = std::fs::write(format!("{dir}/{:03}.ppm", g.ticks), out);
        }
        let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, w, h + 1));
        render::paint_screen(&mut buf, Rect::new(0, 0, w, h), &scr, 1, Glyphs::Hybrid, theme, g.level.palace());
        render::paint_status(&mut buf, Rect::new(0, h, w, 1), &g, "");
        let k = &g.kid;
        println!(
            "== {tok}  scrn={} x={} y={} posn={} action={} face={} bx={} by={} sword={} str={} phase={:?} trobs={} mobs={}",
            k.scrn, k.x, k.y, k.posn, k.action, if k.face == game::FACE_LEFT { "L" } else { "R" },
            k.block_x as i8, k.block_y as i8, k.sword, g.kid_strength, g.phase, g.trobs.len(), g.mobs.len()
        );
        if g.shad.face != guard::NO_CHAR {
            let o = &g.shad;
            println!(
                "   opp id={} scrn={} x={} y={} posn={} action={} face={} by={} sword={} life={:#x} str={}/{} alert={}",
                o.id, o.scrn, o.x, o.y, o.posn, o.action, if o.face == game::FACE_LEFT { "L" } else { "R" },
                o.block_y as i8, o.sword, o.life, g.opp_strength, g.max_opp_str, g.enemy_alert
            );
        }
        let ansi = std::env::var("POB_ANSI").is_ok();
        for y in 0..=h {
            let mut line = String::new();
            for x in 0..w {
                let c = &buf[(x, y)];
                if ansi {
                    let rgb = |c: Color| match c {
                        Color::Rgb(r, g, b) => (r, g, b),
                        Color::White => (230, 230, 230),
                        _ => (0, 0, 0),
                    };
                    let (fr, fg, fb) = rgb(c.fg);
                    let (br, bg, bb) = rgb(c.bg);
                    line.push_str(&format!("\x1b[38;2;{fr};{fg};{fb}m\x1b[48;2;{br};{bg};{bb}m"));
                }
                line.push_str(c.symbol());
            }
            if ansi {
                line.push_str("\x1b[0m");
            }
            println!("{}", line.trim_end());
        }
    }
}

fn handle(app: &mut App, cmd: Cmd) {
    log::info!("cmd {cmd:?}");
    if app.confirm_quit {
        match cmd {
            Cmd::AskQuit | Cmd::Quit => app.quit = true,
            _ => app.confirm_quit = false,
        }
        return;
    }
    let g = &mut app.game;
    match cmd {
        Cmd::Quit => app.quit = true,
        Cmd::AskQuit => app.confirm_quit = true,
        Cmd::Continue => {
            if matches!(g.phase, Phase::Dead(_)) && g.dead_frames > 24 {
                g.restart_level();
            }
        }
        Cmd::RestartLevel => {
            if g.phase == Phase::TimeUp || g.phase == Phase::Won {
                *g = Game::new(1);
            } else {
                g.restart_level();
            }
        }
        Cmd::NextLevel => {
            let n = (g.level.number + 1).min(14);
            g.start_level(n, true);
        }
        Cmd::PrevLevel => {
            let n = g.level.number.saturating_sub(1).max(1);
            g.start_level(n, true);
        }
        Cmd::Glyphs => app.glyphs = app.glyphs.next(),
        Cmd::Theme => {
            app.theme = app.theme.next();
            theme::save(app.theme);
        }
        Cmd::Scale => {
            app.scale = match app.scale {
                0 => 2,
                2 => 1,
                _ => 0,
            }
        }
        Cmd::Pause => app.paused = !app.paused,
    }
}

fn draw(f: &mut ratatui::Frame, app: &mut App, input: &Input) {
    let area = f.area();
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(' ').set_style(Style::default().bg(Color::Black));
            }
        }
    }
    let (w1, h1) = render::cells_needed(1);
    let scale = match app.scale {
        0 => {
            if area.width >= w1 && area.height > h1 {
                1
            } else {
                2
            }
        }
        s => s,
    };
    let (w, h) = render::cells_needed(scale);
    if area.width < w || area.height < h + 1 {
        let msg = format!("Terminal too small: need {}x{} (have {}x{})", w, h + 1, area.width, area.height);
        buf.set_string(0, 0, msg, Style::default().fg(Color::White));
        return;
    }
    bg::draw(&mut app.screen, &app.game);
    let ox = area.x + (area.width - w) / 2;
    let oy = area.y + (area.height - h - 1) / 2;
    render::paint_screen(buf, Rect::new(ox, oy, w, h), &app.screen, scale, app.glyphs, app.theme, app.game.level.palace());
    let mut note = format!("{} {} {}x", app.theme.name(), app.glyphs.name(), scale);
    if input.mode == input::Mode::Timing {
        note.push_str(" keys:timing");
    }
    if app.paused {
        note.push_str(" PAUSED");
    }
    render::paint_status(buf, Rect::new(ox, oy + h, w, 1), &app.game, &note);
    if app.confirm_quit {
        let m = " Quit? Esc again to quit, any other key to stay ";
        let mx = ox + w.saturating_sub(m.len() as u16) / 2;
        buf.set_string(mx, oy + h / 2, m, Style::default().fg(Color::Black).bg(Color::Rgb(230, 200, 120)));
    }
}
