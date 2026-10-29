# Prince of Braille

Asciiverse platformer. Rust + Ratatui + Drawille.
Built from Jordan Mechner's released original Prince of Persia (1989) Apple II
source code — a genuine restoration/reimagining, not a from-scratch clone.

**Status:** Consolidated 2026-07-08 from design conversation; no prior file
existed under this exact name (distinct from "Relief Paint," which briefly
considered and then released this name for its own project).

## Naming note

"Prince of Braille" was briefly considered as a name for the REXPaint-extended
sprite editor project before that editor was named Relief Paint instead — this
name was reserved specifically for this game. Keep the two projects clearly
separate in code, repos, and any outreach.

## Premise

Jordan Mechner released the original Apple II Prince of Persia source code
himself in 2012 — found in a box in his parents' attic, and made public. He's
been one of the most generous and open-handed creators in gaming history about
his own work; a well-documented GitHub repository exists with the original
assembly.

Because the source is genuinely public, building from it is a restoration and
reimagining, not a legally fraught port. The double meaning in the name is
structural, not just wordplay: Prince of *Braille* — rendered *in* Braille
Unicode block characters. The concept is made literal in the medium itself.

## Why this works technically

The original PoP movement system is built on rotoscoped video of Mechner's
brother running and jumping — reduced to the essential geometry needed to run
on Apple II hardware. The prince is recognisable entirely from silhouette: the
running crouch, the sliding stop, the ledge hang, the careful testing step for
floor traps.

Braille dot patterns at sufficient resolution can render that silhouette with
real fidelity, while a texture overlay layer handles the environment — stone
walls, torchlight, dungeon depth. The rotoscoped source gives clean movement
data to animate from; the harder problem is fluid character animation, not
spatial representation.

Mechner reduced human movement to its essential geometry to fit Apple II
constraints. ASCII/Braille reduces visual representation to its essential
characters under a comparable constraint. The two reduction philosophies are
spiritually identical — find the minimum expression that still carries the
truth of the thing.

## Rendering approach — inherits directly from Terminal Orbit's architecture

Terminal Orbit (katalystnord/terminal-orbit) already proved the core rendering
approach needed here, just in a different visual domain:

- **`canvas.rs` (the Drawille/Braille adapter)** — the rendering primitive
  carries over directly. Sub-pixel resolution at 2× horizontal, 4× vertical
  is what gives both games their visual quality.
- **`texture.rs` (raycasted Lambert-lit texture system)** — in Terminal Orbit
  this makes planets feel solid; here the equivalent is dungeon wall texture,
  torchlight gradients, stone-and-shadow environment. Same technical approach,
  different visual domain.
- **`renderer/viewport.rs`** — wireframe ships become the prince's rotoscoped
  silhouette. The original PoP movement system maps onto this renderer more
  naturally than expected.

Terminal Orbit proved the Braille viewport can handle full 3D spatial
rendering with real physics. Prince of Braille is a comparatively simpler
rendering problem — essentially 2.5D side-scrolling — but with far more
demanding character animation requirements than Terminal Orbit needed.

**Texture overlay layering:**
- Stone texture in dungeon walls at one character density
- Shadow gradients from torches at another
- The prince himself in high-contrast Braille dots against the textured
  background

That layering is expected to produce something genuinely atmospheric in a
terminal window — the target "magic moment" is the prince running full speed
toward a gap, planting his foot, and leaping, rendered with motion blur
through character density changes. That's the screenshot that goes everywhere
on release.

## Source and preservation angle

Built directly from the original Apple II assembly source, movement faithful
to Mechner's rotoscoping. This isn't just a game port — it's a legitimate
piece of game preservation history, done with real understanding of how the
original was constructed rather than guessed from playing the finished game.

## Outreach — Jordan Mechner

Not a permission request — the source is already public. A courtesy: telling
him what's being built and why. He's active, thoughtful, and known to be
genuinely interested in what people do with his work. Reasonable expectation
he'd find this project appealing given how directly it honours the original
construction method rather than just skinning a clone.

(Cross-reference: track this contact in the Asciiverse outreach CLAUDE.md
alongside the other developer/rights-holder contacts.)

## What exists (first playable cut, 2026-10-03)

Rust + Ratatui, `cargo run --release -- [LEVEL]` (1-14). It is a port of the
original 6502 routines, run on the original data:

| file | ported from | what |
|------|-------------|------|
| `tools/extract.py` | | reads the reference zip, copies the raw `IMG.*` image tables and `LEVEL*` blueprints to `assets/apple2/`, assembles `SEQTABLE.S` / `FRAMEDEF.S` with a tiny Merlin subset into `src/gen/` |
| `src/apple.rs` | HIRES.S | image tables, 280x192 screen, AND/OR/STA/MASK lay, mirroring, NTSC colour |
| `src/level.rs` | EQ.S | blueprint layout, RDBLOCK |
| `src/seq.rs` | | sequence + frame tables |
| `src/subs.rs` | CTRLSUBS.S, SUBS.S | ANIMCHAR, GETBASEX/GETDIST, SETUPCHAR, GETEDGES, CHECKLEDGE, gravity |
| `src/ctrl.rs` | CTRL.S | all kid control: run, stop, turn, step, stand/run jump, jump up, hang, climb up/down, crouch, crawl, falling, grabbing, landing, impaling, pickups; FightCtrl (strike, block, advance, retreat, sheathe) and GuardCtrl for every character en garde |
| `src/coll.rs` | COLL.S | frame-to-frame barrier collision, bumps, GETFWDDIST, CHECKGATE, CHECKSLICE/CHECKSLICE2, smashing the mirror |
| `src/mover.rs` | MOVER.S, SUBS.S | gates, pressure plates and links, loose floors and falling floor MOBs, spikes, slicers (ADDSLICERS, animslicer), exit door, the level 4 mirror appearing, the level 13 crumbling floors, torches, bubbling flasks |
| `src/bg.rs` | FRAMEADV.S, BGDATA.S | screen drawing with the A/B/C/D/front piece system |
| `src/game.rs` | TOPCTRL.S, AUTO.S | frame loop (NextFrame order: DoKid, DoShad, CHECKSTRIKE, ...), the Char/Kid/Shad/Op buffers, screen cuts, timer, death, level progression |
| `src/guard.rs` | AUTO.S, MISC.S, TOPCTRL.S | guards placed from the level's INFO tables, guard AI (GuardProg, EnGarde, GenFight, the per-program strike/block/advance probabilities), sword contact (CHECKSTRIKE/CHECKSTAB/STABCHAR), strength meters (DECSTR, chgmeters), guards left behind or following you between screens, the skeleton, the shadowman's scripted appearances, the mouse |
| `src/render.rs` | | terminal output (below) |
| `src/sound.rs` | SOUND.S | the speaker effects, synthesised from `tone`'s loop timings, played with rodio |
| `src/input.rs` | SPECIALK.S | keys to joystick + clr flags |

Variable and routine names follow the source, and byte arithmetic wraps the
way the 6502's does, so the port can be read side by side with Mechner's code.

**Rendering, as decided with David:** not Braille-only. The 280x192 screen
is rebuilt pixel for pixel, then coloured by an NTSC model (560-dot line,
palette-bit half-dot shift, 16 colours from a 4-dot window, horizontal
blur). That turns the Apple's dither stipples into the solid light-blue stone
and orange accents a colour monitor showed. Each 2x4 cell then gets:
- the **prince in Braille** (his own pixel colours), which keeps the
  rotoscoped animation fluid;
- the **dungeon in block glyphs** (full, half, quadrant, quarter) with
  fg/bg colour from a 2-means split of the cell, which gives the stone real
  definition; Braille only where no block glyph fits (thin detail).
`g` cycles hybrid / Braille-only / blocks-only for comparison.

**Colour themes** (`src/theme.rs`): David wanted an updated scheme rather
than the old Apple II colours, with a choice. A theme keeps the NTSC
picture's brightness (where the brick faces, mortar and highlights live)
and its accent cue (warm pixels in the dungeon, cool ones in the palace),
and repaints hues by what each pixel is: stone, floor, gate, flame,
potion, sword, prince. Themes: `dusk` (default: slate dungeon, sandstone
palace with teal tiles, amber torchlight), `phosphor` (green with amber),
`amber`, `classic` (the Apple colours unchanged). `c` cycles in game and
the choice is saved to `~/.config/prince-of-braille/theme`;
`--theme NAME` overrides for one run. Below
140x49 cells it renders at half scale (`f` forces 1x or 2x).

**Controls:** arrows / WASD / hjkl, Home/PgUp or q/e for diagonal jumps,
Shift or Space = button (grab, careful step, pick up/drink). With the kitty
keyboard protocol keys are exact (real releases, chords); elsewhere held
keys are inferred from OS key repeat. `r` restart level, `n`/`b` next/prev
level, `p` pause, `v` DOS art or Apple picture, `g` glyph mode, `c`
colour theme and `f` scale (Apple picture), `m` or F8
sound on/off (`--mute` starts without sound), Space skips a princess
scene, Esc Esc quit.

**Debugging:** `RUST_LOG=debug prince-of-braille 2>pob.log` logs every key,
state changes and a 500 ms state snapshot. `--dump "R:12 .:6 U:1"` runs
headless with scripted input and prints frames (`POB_ANSI=1` for colour,
`POB_PPM_DIR=dir` for raw NTSC frames, `POB_AT=scrn,bx,by[,L|R]` to start
the kid somewhere else, `POB_SCALE=2` for the half-size picture); `tools/ansi2png.py` turns an ANSI dump into PNGs.

**Guards and sword fighting** (2026-10-28): ported from AUTO.S, CTRL.S
and MISC.S. As in the original every character routine works on one
"Char" buffer (`ch`) with its opponent in `op`; DoKid loads the kid,
DoShad the guard (`shad`), and both run the same control and physics
code. The guard AI "presses keys" (DoFwd, DoStrike, ...) that GENCTRL
then acts on. Guards come from the blueprint's INFO tables (GdStartBlock,
Face, X, Prog, Seq), with strength and uniform colour per level and
program as in AUTO.S; a red-uniform guard is drawn one pixel over, as the
Apple did. A live guard left behind starts fresh when you come back; a
dead one stays dead. Strength changes go through ChgKidStr/ChgOppStr and
are applied at the end of the frame (chgmeters), so an impact star shows
on whoever was hit that frame. The opponent's meter is at the right of
the status line. `RUST_LOG=info` logs fight events (en garde, strikes,
blocks, stabs, deaths); the 500 ms debug snapshot includes the opponent.

**Where the source left a choice:**
- ADDGUARD calls getgdstrength before it sets `guardprog` for the new
  guard, so a guard's extra strength (program 4 only) comes from the
  previous guard's program. Kept as the original has it.
- The vizier waits for his music (AUTO.S: SongCue) before he goes en
  garde. The Apple froze the game while a song played; here music plays
  over the game, so he waits 28 frames (2.33 s) after his song starts,
  as SDLPoP's meet_Jaffar does.
- RND here is a xorshift, not the original generator: the same
  probabilities, a different sequence.
- Level 7 starts with the kid dropped into screen 1 from screen 17
  (STARTKID's cut, y = -8, block row -1). Traced through the source
  step by step (turn sequence, onground on frame 46, stepfall, freefall
  with setfall 1,15), his base x is 93 as he passes the row 0 floor
  plane: the first pixel of column 2, the open shaft (BlockTable puts
  pixel 86 after the angle in block 2), one pixel short of the floor in
  column 1. Falling on, he reaches terminal velocity and dies on screen
  3. Holding the button through the drop catches the column 1 ledge
  (fallon), and up climbs onto it; the port does the same as the
  source here, so it is left as the source has it. Worth checking
  against a real Apple II or an emulator.

**The shadow, the mirror and slicers** (2026-10-28): the kid's
reflection shows in the level 4 mirror (cropped to it, MISC.S
REFLECTION); a running jump through it smashes the mirror and the
reflection comes to life as the shadowman, who takes all but one of the
kid's strength and runs off. Level 5: the shadow steals the potion once
the gate opens (the original's prerecorded moves). Level 6: he steps on
the plate as you jump the chasm. Level 12: he drops in once the sword is
gone, fights you with what hurts one hurting the other, and merges with
you when you face him with swords down (the kid flashes between the two,
and the phantom bridge appears underfoot on screens 2 and 13). Slicers
run, are drawn and slice, smeared once bloodied. The shadowman is drawn
EORed, as on the Apple.

**Upside-down potion and flashes** (2026-10-28): the upside-down potion
flips the whole picture (HIRES.S INVERTY swaps every line with its
mirror line) until you die or leave the level; controls stay as they
are, as in the original. Screen flashes (lightning: the sword, potions,
the vizier's death, the level 12 merge; red when the kid is hurt) were a
solid lo-res colour for part of each frame on the Apple; here the black
of the picture takes a dimmed copy of that colour for the frame instead
of a full-screen strobe.

**Sound** (2026-10-28): SOUND.S's effects (footsteps, plates, gates,
the exit, splats, sword clashes, slicer jaws, ...) rebuilt as the Apple
speaker made them: `tone` flips the one-bit speaker `duration` times
with a delay loop between flips, so each effect is a square wave whose
half-period is that loop's cycle count at 1.023 MHz. Rendered once at
start-up and played with rodio, as in the other Asciiverse games. The
drink and upstairs events were music cues in the original and stay
silent.

**The princess's room** (2026-10-28): `src/cut.rs` plays SUBS.S's
scenes (PlayCut0-8) on the game's own character buffers and animchar:
the vizier turning the hourglass before a new game on level 1, the cuts
before levels 2, 4, 6, 8, 9 and 12 (cut 5 turns ominous when the sand is
almost out), the tragic ending when time runs out and the happy ending
on reaching her. Start positions, sequences, SPEED changes and the
hourglass's state from the time left are as in the source; Space skips a
scene. `src/room.rs` draws the room (2026-10-29): the DOS version's
picture in its own colours, from PrinceJS (Unlicense,
`tools/dos_room.py` writes `assets/dos/room.bin`): the room, the bed,
the hourglass and its sand, the stars and the two front pillars, where
SDLPoP and PrinceJS put them. The characters and torch flames are the
Apple images over it. DOS x is the Apple's times 8/7 (SDLPoP
calc_screen_x_coord); the DOS room stands its characters 16 lines lower,
so the picture is drawn from its line 8 and the characters 8 lines lower
than the Apple had them. The night sky is lifted just enough to show.
Frame pacing is an estimate: PAUSE is about 1.26 ms
per unit of SPEED, the drawing time around it is a guess that makes the
usual SPEED 12 run at the game's 12 frames a second. `POB_CUT=n` with
`--dump` starts scene n for testing.

**Music** (2026-10-28): the DOS version's songs as MP3s from PrinceJS
(Unlicense), in `assets/music/`, embedded and played with rodio. `src/music.rs`
cues them where the Apple code cued its songs, and where PrinceJS plays
them: danger on starting level 1, accident, heroic or shadow death, the
sword and a guard killed (victory), the potions (small, big, float), the
next level (prince; the shadow's tune on level 4), the shadow moments on
levels 4, 6 and 12, meeting and killing Jaffar, and the princess scenes.
A song plays over the game; `m` mutes it with the rest of the sound.

**Clock** (2026-10-28): KEEPTIME, GETMINLEFT and SHOWTIME. It counts
frames up, 725 to a minute (SPECIALK.S `min`), stops while the kid is
dead, shows the time at the timetable's marks, after the level name and
when the vizier dies, counts down in seconds through the final minute,
and runs out five frames after t = 0. `n` cuts the time left to 15
minutes, as the original's level skip did (SHORTENTIME).

**The demo** (2026-10-29): `--demo` plays level 0, the original's attract
mode (SUBS.S DemoProg1, CTRL.S DemoCtrl): the kid runs, jumps two pits,
grabs a ledge and climbs, then fights the first guard on his own (as
guard program 10, the guard as 11). No clock and no music, as in the
original; it ends at EndDemo or when the kid dies, and any key starts a
new game (Esc leaves). With no input it plays the same each time, so the
demo seeds the RNG with 5, a seed where the kid wins. It is the catalog
preview. `POB_SEED=n` with `--dump` tries another seed.

**Characters in the scene** (2026-10-28): CROPCHAR, INDEXCHAR and
QUICKFLOOR. Each character is cropped (under a solid floor, left of a
panel or block, climbing the stairs) and drawn with the block INDEXCHAR
gives him; the mid plane goes block by block, bottom row up, left to
right, each block's QUICKFLOOR-marked floorpiece before the characters
indexed to it, as FRAMEADV.S FAST does. Impact stars come last. Mirrored
images now end just left of x (MLAY subtracts the width from XCO): the
port had drawn every right-facing character 7 pixels too far right since
the first cut.

**The level 10 panel** (settled 2026-10-29): facing right into a palace
panel (level 10 screen 1, after the bump) the kid stands inside the
panel's picture, as COLL.S puts him; the Apple and the DOS layering both
hide him behind its front. The DOS renderer leaves that panel's front out
while most of him is in its block, so he stays in view (tested).

**Direction (David, 2026-10-29):** a good terminal Prince of Persia, not
an Apple II emulation. The source decides how it plays (moves, timing,
guards, levels); how it looks, sounds and handles is designed for the
terminal, and where the two disagree the player wins. Bugs in the
original that hurt play get fixed, as SDLPoP's optional fixes do. Plan,
in order: the room (done), the DOS renderer (done, below; David chose
the DOS art over new terminal art), the original's bugs (done, below),
and controls that work from key repeat.

**The original's bugs** (2026-10-29): SDLPoP 1.16's fixes, checked one
by one against our port and applied where the bug is in the Apple code
too, each commented at its place: fix_two_coll_bug (COLLISIONS checks
both barrier edges), fix_infinite_down_bug (CHECKSPIKES stops one row
below the screen), fix_jump_distance_at_edge (DOJUMPHIGH),
fix_painless_fall_on_guard (FIRSTGUARD), fix_wall_bump_triggers_tile_below
and fix_stand_on_thin_air (byte patches to bumpfall and standup in
`seq::patched`, plus CHECKFLOOR on the stand-up frames),
fix_press_through_closed_gates (CHECKPRESS rereads his block),
fix_grab_falling_speed (GRABSPEED 30), fix_skeleton_chomper_blood,
fix_move_after_drink (GENCTRL), fix_guard_following_through_closed_gates
(cutguards), fix_safe_landing_on_spikes (HITFLR), and fix_gate_sounds,
which came with the lowering-gate tick (SUBS.S ADDLOWERSOUND) the port
had left out. Not taken: fix_edge_distance_check_when_climbing (the Apple
DOJUMPHANG never had the extra check), fix_loose_left_of_potion (a DOS
drawing bug), enable_freeze_time_during_end_music (nothing here waits
for the music), fix_gate_drawing_bug and fix_bigpillar_climb (off in
SDLPoP too). enable_crouch_after_climbing waits for the controls work.

**The DOS renderer** (2026-10-29, `src/dos.rs`): the game is drawn with
the DOS version's art from PrinceJS (Unlicense; `tools/dos_art.py` copies
its sheets to `assets/dos/` and writes `src/gen/dos_atlas.rs`), put
together as PrinceJS builds a room: per block a 60x79 back and front
image at x = 32 col, y = 63 row - 13, all backs (lower rooms first, rows
bottom up, left to right), then the characters, then all fronts. The
picture is 320x192 and `render::paint_dos` fits it to the window at any
size (a DOS pixel 1.2 dots tall), characters in Braille. What the engine
knows drives it: our frame tables match the DOS ones, so a character's
DOS x is ours times 8/7 and his sprite is `kid-<frame>`, `guard-<colour>-
<frame - 150>` (colour per level and room from PrinceJS's maps),
`shadow-`, `skeleton-`, `jaffar-` (level 13), `fatguard-` (level 6);
swords from the DOS sword table. Gates, spikes, loose floors, slicers,
torches, potions, the sword, plates and the exit door follow the blocks'
states. A kid hanging facing right goes behind the floor he hangs from
(PrinceJS maskTile, on the floors QUICKFLOOR marks); climbing up he has
the `kid-135r`... frames. A hurt character shows the DOS splash (a
guard's tinted his colour). The mirror is PrinceJS's: a floor with the
glass on it, the kid's reflection mirrored about it and a cover hiding
it outside. The princess's scenes are drawn the same way (`room::picture`):
the DOS room, `princess-<frame>`, `vizier-<frame - 47>`, `mouse-<frame -
185>`, the DOS flames. `v` switches to the Apple picture (`POB_VIEW=apple`
in a dump), where `c`, `f` and the themes still apply; `POB_SIZE=WxH`
sets a dump's size, `POB_MIRROR` puts up the level 4 mirror.
