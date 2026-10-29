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
mod cut;
mod dos;
mod coll;
mod ctrl;
mod game;
mod guard;
mod input;
mod level;
mod music;
mod mover;
mod render;
mod room;
mod save;
mod seq;
mod sound;
mod subs;
mod theme;
mod title;

use apple::Screen;
use game::{Game, Phase, FPS};
use input::{Cmd, Input};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
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
    /// draw the Apple picture instead of the DOS art (`v`)
    apple: bool,
    theme: theme::Theme,
    /// 0 = fit automatically, else forced 1 or 2
    scale: i32,
    paused: bool,
    confirm_quit: bool,
    quit: bool,
    audio: sound::Audio,
    /// the saved game, if there is one
    saved: Option<save::Save>,
    /// the title or the end on the screen, instead of the game
    title: Option<title::Title>,
    /// the game began at the title: its story and the demo go round
    /// until a key is pressed (`--demo` alone just plays the demo)
    attract: bool,
}

impl App {
    fn show_title(&mut self, part: title::Part) {
        self.audio.stop_song();
        self.title = Some(title::Title::new(part));
    }

    /// A new game on level `n`; level 1 starts with the danger theme.
    fn play(&mut self, n: u8) {
        log::info!("new game on level {n}");
        self.title = None;
        self.audio.stop_song();
        self.game = Game::new(n);
        if n == 1 {
            self.audio.start_song(music::Song::Danger);
        }
    }

    /// At the title, in its story or in the demo: any key plays.
    fn in_attract(&self) -> bool {
        self.title.is_some()
            || self.game.level.number == 0
            || self.game.cut.as_ref().is_some_and(|c| c.after == cut::After::Attract)
    }

    /// A key at the title, in its story or the demo.
    fn attract_key(&mut self, k: event::KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Char('l') if ctrl => handle(self, Cmd::LoadGame),
            KeyCode::Char('m') | KeyCode::F(8) => handle(self, Cmd::Mute),
            KeyCode::Modifier(_) => {}
            _ => self.attract_any(),
        }
    }

    /// Any key (or pad button) at the title, in its story or the demo.
    fn attract_any(&mut self) {
        // a key in the end goes on to the title, as in SDLPoP
        if self.title.as_ref().is_some_and(|t| t.part == title::Part::Epilogue) {
            self.show_title(title::Part::Opening)
        } else {
            self.play(1)
        }
    }

    /// A gamepad press outside the controls: at the title any button
    /// plays and Back quits; in a game Start pauses, Back asks to quit,
    /// and the others go on after a death, as Space does.
    fn pad_press(&mut self, p: input::PadPress) {
        use input::PadPress;
        if self.in_attract() {
            match p {
                PadPress::Back => self.quit = true,
                _ => self.attract_any(),
            }
            return;
        }
        handle(
            self,
            match p {
                PadPress::Start => Cmd::Pause,
                PadPress::Back => Cmd::AskQuit,
                PadPress::Face => Cmd::Continue,
            },
        );
    }

    /// The next step of the title, its story, the demo and the end.
    fn attract_step(&mut self) {
        if let Some(t) = &mut self.title {
            if let Some(s) = t.tick() {
                self.audio.start_song(s);
            }
            if t.done() {
                match t.part {
                    title::Part::Opening => {
                        // the vizier in the princess's room
                        self.title = None;
                        self.game = Game::new(1);
                        self.game.playcut(0, 1, cut::After::Attract);
                    }
                    title::Part::Story => {
                        self.title = None;
                        self.game = Game::new(0);
                    }
                    title::Part::Epilogue => self.show_title(title::Part::Opening),
                }
            }
            return;
        }
        let g = &self.game;
        if g.demo_over {
            if g.level.number != 0 {
                self.show_title(title::Part::Story);
            } else if self.attract {
                self.show_title(title::Part::Opening);
            } else {
                self.quit = true;
            }
        } else if g.phase == Phase::Won {
            self.attract = true;
            self.show_title(title::Part::Epilogue);
        } else if g.phase == Phase::TimeUp {
            // SDLPoP expired: the tragic scene, then the title
            self.attract = true;
            self.show_title(title::Part::Opening);
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: prince-of-braille [--theme dusk|phosphor|amber|classic] [--mute] [--demo] [LEVEL]   (level 1-14; with none, the title)");
    std::process::exit(2);
}

fn main() -> io::Result<()> {
    env_logger::init();
    // None: start at the title
    let mut level: Option<u8> = None;
    let mut dump: Option<String> = None;
    let mut theme = theme::load().unwrap_or(theme::Theme::Dusk);
    let mut mute = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => usage(),
            "--dump" => dump = Some(args.next().unwrap_or_default()),
            "--mute" => mute = true,
            // the original's attract mode: level 0 plays itself
            "--demo" => level = Some(0),
            "--theme" => {
                theme = args.next().as_deref().and_then(theme::Theme::from_name).unwrap_or_else(|| usage());
            }
            n => level = Some(n.parse().ok().filter(|v| (1..=14).contains(v)).unwrap_or_else(|| usage())),
        }
    }
    if let Some(script) = dump {
        dump_frames(level.unwrap_or(1), &script, theme);
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
    log::info!("start level={level:?} exact_keys={exact}");
    let result = run(&mut terminal, level, exact, theme, mute);
    if exact {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, level: Option<u8>, exact: bool, theme: theme::Theme, mute: bool) -> io::Result<()> {
    let mut app = App {
        game: Game::new(level.unwrap_or(0)),
        screen: Screen::default(),
        glyphs: Glyphs::Hybrid,
        apple: false,
        theme,
        scale: 0,
        paused: false,
        confirm_quit: false,
        quit: false,
        audio: sound::Audio::new(mute),
        saved: save::load(),
        title: None,
        attract: level.is_none(),
    };
    match level {
        None => app.show_title(title::Part::Opening),
        Some(0) => {}
        Some(n) => app.play(n),
    }
    // a game started on a level isn't saved until it reaches another
    let mut saved_level = level.unwrap_or(0);
    let mut input = Input::new(exact);
    input.open_pad();
    log::info!("gamepad {:?}", input.pad_name);
    let tick = Duration::from_secs(1) / FPS;
    let mut next = Instant::now() + tick;
    let mut last_snapshot = Instant::now();
    while !app.quit {
        terminal.draw(|f| draw(f, &mut app, &input))?;
        while event::poll(next.saturating_duration_since(Instant::now()))? {
            if let Event::Key(k) = event::read()? {
                log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                if app.in_attract() {
                    if k.kind == KeyEventKind::Press {
                        app.attract_key(k);
                    }
                    continue;
                }
                input.on_key(k, Instant::now());
            }
        }
        input.poll_pad((app.game.kid.action, app.game.kid.posn));
        for p in std::mem::take(&mut input.pad_presses) {
            log::debug!("pad {p:?}");
            app.pad_press(p);
        }
        for cmd in std::mem::take(&mut input.cmds) {
            handle(&mut app, cmd);
        }
        let now = Instant::now();
        let mut steps = 0;
        while now >= next && steps < 3 {
            if app.title.is_some() {
                app.attract_step();
            } else if !app.paused && !app.confirm_quit {
                let c = input.controls(now);
                app.game.tick(c);
                app.audio.play(&app.game.sounds);
                if let Some(s) = app.game.song_start {
                    app.audio.start_song(s);
                }
                app.attract_step();
                // reaching a level saves the game, as Ctrl-G there would
                let g = &app.game;
                if g.level.number != saved_level && g.cut.is_none() && g.phase == Phase::Playing {
                    saved_level = g.level.number;
                    if let Some(s) = save::Save::of(g) {
                        save::store(s);
                        app.saved = Some(s);
                    }
                }
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
    if let Some(r) = std::env::var("POB_SEED").ok().and_then(|v| v.parse().ok()) {
        g.rng = r;
    }
    // POB_CUT=n: start with princess scene n (as before entering `level`)
    if let Some(n) = std::env::var("POB_CUT").ok().and_then(|v| v.parse().ok()) {
        g.playcut(n, level, cut::After::Level(level));
    }
    // POB_MIRROR: level 4's mirror already up, as when the exit has opened
    if std::env::var("POB_MIRROR").is_ok() && level == 4 {
        g.level.set_type(level::Level::index(4, 4, 0), level::obj::MIRROR);
        g.exitopen = 1;
    }
    if let Some(at) = std::env::var("POB_AT").ok().and_then(|v| parse_at(&v)) {
        g.place_kid(at.0, at.1, at.2, at.3);
    }
    // POB_TITLE=opening|story|epilogue: the title's pictures instead
    let mut title = std::env::var("POB_TITLE").ok().map(|v| {
        title::Title::new(match v.as_str() {
            "story" => title::Part::Story,
            "epilogue" => title::Part::Epilogue,
            _ => title::Part::Opening,
        })
    });
    let mut scr = Screen::default();
    // POB_SCALE=2: the half-size picture, as on a small terminal
    let scale = std::env::var("POB_SCALE").ok().and_then(|v| v.parse().ok()).filter(|s| *s == 2).unwrap_or(1);
    // POB_VIEW=apple: the Apple picture; else the DOS art in POB_SIZE
    // cells (WxH, the status line below), by default the Apple's size
    let apple_view = std::env::var("POB_VIEW").is_ok_and(|v| v == "apple");
    let (w, h) = match std::env::var("POB_SIZE").ok().and_then(|v| {
        let (a, b) = v.split_once('x')?;
        Some((a.parse().ok()?, b.parse::<u16>().ok()?.saturating_sub(1)))
    }) {
        Some(size) if !apple_view => size,
        _ => render::cells_needed(scale),
    };
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
        if let Some(t) = &mut title {
            for _ in 0..n {
                t.tick();
            }
            let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, w, h + 1));
            draw_title(&mut buf, Rect::new(0, 0, w, h + 1), t, Glyphs::Hybrid, None);
            println!("== {tok}  title {:?}", t.part);
            print_buf(&buf, w, h);
            continue;
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
        if apple_view {
            render::paint_screen(&mut buf, Rect::new(0, 0, w, h), &scr, scale, Glyphs::Hybrid, theme, g.level.palace() || g.cut.is_some(), g.cut.as_ref().map(room::backdrop).as_ref());
        } else {
            let r = render::dos_fit(Rect::new(0, 0, w, h), dos::H);
            let pic = match &g.cut {
                Some(cut) => room::picture(&g, cut),
                None => dos::picture(&g),
            };
            render::paint_dos(&mut buf, r, &pic, Glyphs::Hybrid);
        }
        render::paint_status(&mut buf, Rect::new(0, h, w, 1), &g, "", None);
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
        print_buf(&buf, w, h);
    }
}


/// Print a dump's frame (POB_ANSI=1 in colour).
fn print_buf(buf: &ratatui::buffer::Buffer, w: u16, h: u16) {
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

/// POB_AT="scrn,bx,by[,L|R]": where the kid starts a --dump run.
fn parse_at(v: &str) -> Option<(u8, u8, u8, u8)> {
    let p: Vec<&str> = v.split(',').collect();
    let n = |i: usize| p.get(i)?.trim().parse::<u8>().ok();
    let face = if p.get(3).is_some_and(|f| f.trim() == "L") { game::FACE_LEFT } else { game::FACE_RIGHT };
    Some((n(0)?, n(1)?, n(2)?, face))
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
        Cmd::RestartLevel => g.restart_level(),
        Cmd::NextLevel => {
            let n = (g.level.number + 1).min(14);
            g.shortentime();
            g.start_level(n, true);
        }
        Cmd::PrevLevel => {
            let n = g.level.number.saturating_sub(1).max(1);
            g.start_level(n, true);
        }
        Cmd::Glyphs => app.glyphs = app.glyphs.next(),
        Cmd::View => app.apple = !app.apple,
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
        Cmd::SaveGame => {
            if g.cut.is_some() || !matches!(g.phase, Phase::Playing | Phase::Dead(_)) {
                return;
            }
            let msg = match save::Save::of(g) {
                Some(s) if save::store(s) => {
                    app.saved = Some(s);
                    "GAME SAVED"
                }
                Some(_) => "UNABLE TO SAVE GAME",
                None if g.level.number < *save::LEVELS.start() => "SAVING STARTS ON LEVEL 3",
                None => return,
            };
            g.show_message(msg.into(), 2 * FPS);
        }
        Cmd::LoadGame => {
            if let Some(s) = app.saved.filter(|_| app.in_attract()) {
                log::info!("resume {s:?}");
                app.title = None;
                app.audio.stop_song();
                app.game = s.resume();
            }
        }
        Cmd::Mute => {
            app.audio.muted = !app.audio.muted;
            if app.audio.muted {
                app.audio.stop_song();
            }
            log::info!("sound muted {}", app.audio.muted);
        }
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
    if let Some(t) = &app.title {
        draw_title(buf, area, t, app.glyphs, app.saved.map(|s| s.level));
        return;
    }
    if !app.apple {
        draw_dos(buf, area, app, input);
        return;
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
    render::paint_screen(buf, Rect::new(ox, oy, w, h), &app.screen, scale, app.glyphs, app.theme, app.game.level.palace() || app.game.cut.is_some(), app.game.cut.as_ref().map(room::backdrop).as_ref());
    if let Some(c) = app.game.flash() {
        render::paint_flash(buf, Rect::new(ox, oy, w, h), c);
    }
    let note = status_note(app, input, &format!("{} {} {}x", app.theme.name(), app.glyphs.name(), scale));
    render::paint_status(buf, Rect::new(ox, oy + h, w, 1), &app.game, &note, app.saved.map(|s| s.level));
    confirm_quit(buf, app, ox, oy, w, h);
}

/// The title, the story screens or the end, with what a key does below.
fn draw_title(buf: &mut ratatui::buffer::Buffer, area: Rect, t: &title::Title, glyphs: Glyphs, resume: Option<u8>) {
    let r = render::dos_fit(Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)), title::LINES);
    render::paint_dos(buf, r, &t.picture(), glyphs);
    t.paint_words(buf, r);
    let hint = if t.part == title::Part::Epilogue { "Any key goes on".to_string() } else { render::attract_hint(resume) };
    let w = hint.chars().count() as u16;
    let style = Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Black);
    buf.set_string(r.x + r.width.saturating_sub(w) / 2, r.y + r.height, hint, style);
}

/// The game in the DOS art, as large as the window allows.
fn draw_dos(buf: &mut ratatui::buffer::Buffer, area: Rect, app: &App, input: &Input) {
    let pic = match &app.game.cut {
        Some(cut) => room::picture(&app.game, cut),
        None => dos::picture(&app.game),
    };
    let r = render::dos_fit(Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)), dos::H);
    render::paint_dos(buf, r, &pic, app.glyphs);
    if let Some(c) = app.game.flash() {
        render::paint_flash(buf, r, c);
    }
    let note = status_note(app, input, app.glyphs.name());
    render::paint_status(buf, Rect::new(r.x, r.y + r.height, r.width, 1), &app.game, &note, app.saved.map(|s| s.level));
    confirm_quit(buf, app, r.x, r.y, r.width, r.height);
}

fn status_note(app: &App, input: &Input, view: &str) -> String {
    let mut note = view.to_string();
    if input.pad_name.is_some() {
        note.push_str(" pad");
    } else if input.mode == input::Mode::Timing {
        note.push_str(" keys:timing");
    }
    if app.audio.muted {
        note.push_str(" mute");
    }
    if app.paused {
        note.push_str(" PAUSED");
    }
    note
}

fn confirm_quit(buf: &mut ratatui::buffer::Buffer, app: &App, ox: u16, oy: u16, w: u16, h: u16) {
    if app.confirm_quit {
        let m = " Quit? Esc again to quit, any other key to stay ";
        let mx = ox + w.saturating_sub(m.len() as u16) / 2;
        buf.set_string(mx, oy + h / 2, m, Style::default().fg(Color::Black).bg(Color::Rgb(230, 200, 120)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(level: Option<u8>) -> App {
        let mut a = App {
            game: Game::new(level.unwrap_or(0)),
            screen: Screen::default(),
            glyphs: Glyphs::Hybrid,
            apple: false,
            theme: theme::Theme::Dusk,
            scale: 0,
            paused: false,
            confirm_quit: false,
            quit: false,
            audio: sound::Audio::new(true),
            saved: None,
            title: None,
            attract: level.is_none(),
        };
        if let Some(n) = level {
            a.play(n);
        }
        a
    }

    #[test]
    fn winning_ends_with_the_epilogue_then_the_title() {
        let mut a = app(Some(14));
        a.game.phase = Phase::Won;
        a.attract_step();
        assert_eq!(a.title.as_ref().map(|t| t.part), Some(title::Part::Epilogue));
        // a key there goes on to the title
        a.attract_key(event::KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(a.title.as_ref().map(|t| t.part), Some(title::Part::Opening));
        assert!(a.attract && !a.quit);
    }

    #[test]
    fn time_up_goes_back_to_the_title_and_a_key_plays_level_1() {
        let mut a = app(Some(5));
        a.game.phase = Phase::TimeUp;
        a.attract_step();
        assert_eq!(a.title.as_ref().map(|t| t.part), Some(title::Part::Opening));
        a.attract_key(event::KeyEvent::from(KeyCode::Char(' ')));
        assert!(a.title.is_none());
        assert_eq!((a.game.level.number, a.game.phase), (1, Phase::Playing));
        assert!(a.game.cut.is_none());
    }

    #[test]
    fn the_demo_alone_quits_when_it_ends() {
        let mut a = app(Some(0));
        a.game.demo_over = true;
        a.attract_step();
        assert!(a.quit);
    }
}
