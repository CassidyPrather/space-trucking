//! **The `Esc` menu: the controls that were never in the room.**
//!
//! Pause, fast-forward, mute, a new run, and the Guild's delivery tally
//! are not things aboard a freighter — they are things you do *to* the
//! game. For most of this project's life all but the new run were bolted
//! to the console face anyway, three icon buttons and a lamp strip
//! screwed to a wall, which made the cabin claim to contain its own
//! volume knob. The wall came down (`crate::console`, which keeps the
//! hardware on the shelf); the controls landed here, on an overlay that
//! is honestly an overlay.
//!
//! **Zero text, like everything else, but for one label and the keycaps**
//! (DESIGN.md's law: nothing renders a string except the version corner,
//! the new-run label and the keys page's keycaps). The icons are
//! *stamped* — a bit per cell, one `u16`
//! per row, spawned as plain colored nodes. It is the console face's
//! stroke-and-primitive discipline one dimension down: no font, no glyph
//! atlas, no words, and the same palette roles doing the same jobs —
//! AMBER for a live function, a lamp under each button for its state, a
//! slash across the speaker so mute carries *shape* and never hue alone.
//!
//! **The new run is the exception, and it says so in words.** Every
//! icon here toggles something the next press toggles back. The new run
//! throws the run away, and nothing brings it back. A wordless button
//! that ends the run can be pressed by someone who thought it meant
//! something else, so it wears a plain label as insurance against that
//! accident. It is a different kind of control and is drawn as one: a
//! wide bar at the foot of the panel, below the tally and out of the
//! icon row, framed in the refusal red, with no lamp because it has no
//! state. It used to be the `R` key, which is gone. A key is pressed by
//! a hand that was reaching for its neighbour, and no key starts a new
//! run now.
//!
//! **The sim stays the authority.** Nothing here freezes a frame. A
//! click sets an edge; `advance` folds that edge into the `InputFrame`
//! exactly where the console icons used to fold in, and the sim decides
//! what pausing means. The world keeps turning while the menu stands —
//! a menu left open is not a paused game unless the player says so.
//!
//! **A stopped world says so.** Every reading on the panel lives and
//! dies with the panel, which was fair for three lamps nobody has to
//! consult and wrong for the one state that changes what every click
//! does. A paused game and a running one were the same picture: the
//! cabin goes on breathing, the crosshair goes on finding things, the
//! hover glow goes on answering, and the presses land nowhere. A player
//! lost an evening to it and filed the bug against the click. So the
//! pause bars leave the panel — stamped a second time under the
//! crosshair, at the aim point, where a press that does nothing is
//! noticed. The chevrons go with them: a world running at sixteen times
//! speed is a world whose behaviour nobody can account for either.
//!
//! The mark is not a HUD. It is on the screen only while the state it
//! names is on, and a world running as it should draws nothing at all.
//! Which state it is, it says in shape — bars against chevrons, the
//! panel's own two icons — so it is still read by a player who cannot
//! tell amber from anything.
//!
//! Mute keeps no such mark and wants none. A muted ship is silent, and
//! the silence is the whole reading: the state announces itself in the
//! medium it acts on. Pause and fast-forward act on a medium that says
//! nothing back.
//!
//! **The keys page** (docs/BAY.md, "Lift, and the keys"). The owner asked
//! for one: the carry's four keys — turn either way, raise, lower — are
//! rebound here, and kept beside the save (`crate::keys`). A fourth face
//! on the icon row, a stamped keyboard, swaps the panel's lower half
//! between the tally and the keys, and its lamp says which is showing.
//! The page is a table: one row per action, the action drawn as a glyph
//! the way every control here is drawn, and beside it a **keycap with the
//! key's name on it** — the second place the menu prints words, because a
//! key's name is text by nature and a keyboard's own caps are where a
//! player has always read it. Click a keycap and it goes blank and amber,
//! waiting; press a key and it is bound. A key another action holds
//! swaps the two, so nothing is ever unbound and no key does two things.
//! A key the game keeps for itself is refused: the cap stays waiting and
//! wears the refusal slash for a moment, shape and not hue alone. `Esc`
//! while a cap waits stops the waiting and leaves the menu standing; the
//! key that answers a waiting cap goes to the cap and to nothing else in
//! the game. Under the table a keycap reading **Defaults** puts every
//! key back.
//!
//! `Esc` semantics, preserved rather than replaced: focused at a
//! station, `Esc` steps out first (the room before the meta); roaming,
//! `Esc` opens this and frees the cursor — which is exactly the parking
//! `Esc` always did, only now there is something to click. `Esc` again,
//! or a click on the bare scrim, puts it away and takes the cursor back.
//! A waiting keycap is the one thing nearer than the menu: its `Esc` is
//! the keycap's.

// The glyph rows ARE the pictures: a separator inside `0b111011100`
// breaks the correspondence between the literal and the shape it draws.
// Same bargain `palette` strikes with its hex.
#![allow(clippy::unreadable_literal)]

use bevy::prelude::*;

use crate::console::DELIVERY_LAMPS;
use crate::keys::{Action, Bindings};
use crate::rig::{CameraRig, Mode};
use crate::{Phase, Shell, palette};

/// Icon cell edge, window pixels. The menu lives outside the crunch (as
/// the crosshair and the version corner do), so it stays crisp — but it
/// is still built out of square cells, because the game is.
const CELL: f32 = 4.0;

/// Button face size, and the lamp bar beneath each glyph.
const FACE: f32 = 56.0;
const LAMP_W: f32 = 24.0;
const LAMP_H: f32 = 4.0;

/// Tally pip size and the gap between pips.
const PIP: f32 = 12.0;
const PIP_GAP: f32 = 8.0;

/// How far the mark hangs below the crosshair, window pixels. Clear of
/// the aim dot, inside the same glance. The mark itself is stamped at
/// [`CELL`] like everything else here: it is not a second drawing of
/// the pause icon, it is the panel's own icon standing somewhere else.
const TELL_DROP: f32 = 20.0;

/// A hovered but sleeping lamp wakes this far — interactable, not
/// active. The exact courtesy the console face's buttons paid.
const HOVER_WAKE: f32 = 0.18;

/// The speaker's honest resting level: audible is a soft green, not a
/// hot one.
const SPEAKER_LEVEL: f32 = 0.45;

/// The new-run bar's height, the extra air above it, and its label's
/// size. The bar spans the panel, so its width is the panel's.
const NEW_RUN_H: f32 = 32.0;
const NEW_RUN_GAP: f32 = 10.0;
const NEW_RUN_TEXT: f32 = 16.0;

/// The new-run bar's label. Plain and short, and kept in this one place:
/// DESIGN.md asks that text the game cannot avoid be translatable, and a
/// translation of this is a change to this line.
const NEW_RUN: &str = "New run";

/// A keycap on the keys page: its size, the depth of its bottom lip (a
/// key is drawn as a key, standing on a thicker edge than its other
/// three), and the size its name is printed at — the new-run label's.
const CAP_W: f32 = 72.0;
const CAP_H: f32 = 40.0;
const CAP_LIP: f32 = 5.0;
const CAP_TEXT: f32 = 16.0;

/// The gap between the keys page's rows, and between a row's glyph and
/// its keycap.
const ROW_GAP: f32 = 8.0;
const ROW_SPLIT: f32 = 16.0;

/// The keys page's one word: the keycap that puts every key back on its
/// default. A key's name is printed on its cap because it is text by
/// nature; this is printed in the same style for the same reason the
/// new-run bar wears its label — a wordless control that throws a
/// player's bindings away can be pressed by someone who thought it meant
/// something else. One string in one place, so it stays translatable.
const DEFAULTS: &str = "Defaults";

/// How long a refused key's slash stays on a waiting keycap, seconds:
/// the 2D juice's refusal flash, kept.
const REFUSAL_LEN: f32 = 0.45;

/// A stamped icon: one bit per cell, most significant bit leftmost, one
/// row per `u16`. Drawn as runs of set bits, so a bar is one node.
struct Glyph {
    w: u8,
    rows: &'static [u16],
}

/// Pause: two upright bars — the same two the console face etched.
const PAUSE: Glyph = Glyph {
    w: 9,
    rows: &[
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
        0b011101110,
    ],
};

/// Fast-forward: a double chevron pointing right, the warp icon's own
/// shape. Dev-gated, exactly as the console button was.
const FAST: Glyph = Glyph {
    w: 10,
    rows: &[
        0b1100110000,
        0b0110011000,
        0b0011001100,
        0b0001100110,
        0b0000110011,
        0b0001100110,
        0b0011001100,
        0b0110011000,
        0b1100110000,
    ],
};

/// Speaker: a box body and a horn opening to the right.
const SPEAKER: Glyph = Glyph {
    w: 9,
    rows: &[
        0b000000100,
        0b000001100,
        0b000011100,
        0b111111100,
        0b111111100,
        0b111111100,
        0b000011100,
        0b000001100,
        0b000000100,
    ],
};

/// The refusal bar over the speaker: mute is a shape before it is a
/// color, so a player who cannot tell red from green can still read it.
const SLASH: Glyph = Glyph {
    w: 9,
    rows: &[
        0b000000011,
        0b000000110,
        0b000001100,
        0b000011000,
        0b000110000,
        0b001100000,
        0b011000000,
        0b110000000,
        0b100000000,
    ],
};

/// The keys page's face on the icon row: a keyboard, its frame and two
/// rows of keys over a space bar.
const KEYBOARD: Glyph = Glyph {
    w: 11,
    rows: &[
        0b11111111111,
        0b10000000001,
        0b10101010101,
        0b10000000001,
        0b10101010101,
        0b10000000001,
        0b10011111001,
        0b10000000001,
        0b11111111111,
    ],
};

/// **Turn counter-clockwise**: a loop open at the top right, its arrow
/// on the right-hand end pointing up — the way the right-hand side of
/// anything turning counter-clockwise goes.
const TURN_CCW: Glyph = Glyph {
    w: 13,
    rows: &[
        0b0001110000000,
        0b0111110001100,
        0b0110000011110,
        0b1100000111111,
        0b1100000001100,
        0b1100000001100,
        0b1100000001100,
        0b1100000001100,
        0b0110000011000,
        0b0111111111000,
        0b0001111100000,
    ],
};

/// **Turn clockwise**: [`TURN_CCW`] in the mirror.
const TURN_CW: Glyph = Glyph {
    w: 13,
    rows: &[
        0b0000000111000,
        0b0011000111110,
        0b0111100000110,
        0b1111110000011,
        0b0011000000011,
        0b0011000000011,
        0b0011000000011,
        0b0011000000011,
        0b0001100000110,
        0b0001111111110,
        0b0000011111000,
    ],
};

/// **Raise**: an arrow up and away from the bar it stands on, the
/// surface a lift is measured from.
const RAISE: Glyph = Glyph {
    w: 10,
    rows: &[
        0b0000110000,
        0b0001111000,
        0b0011111100,
        0b0110110110,
        0b0000110000,
        0b0000110000,
        0b0000000000,
        0b1111111111,
        0b1111111111,
    ],
};

/// **Lower**: the arrow down onto the bar.
const LOWER: Glyph = Glyph {
    w: 10,
    rows: &[
        0b0000110000,
        0b0000110000,
        0b0110110110,
        0b0011111100,
        0b0001111000,
        0b0000110000,
        0b0000000000,
        0b1111111111,
        0b1111111111,
    ],
};

/// The glyph an action is drawn as on the keys page.
const fn glyph_of(action: Action) -> &'static Glyph {
    match action {
        Action::TurnCcw => &TURN_CCW,
        Action::TurnCw => &TURN_CW,
        Action::Raise => &RAISE,
        Action::Lower => &LOWER,
    }
}

/// Which meta-control a face, glyph cell, or lamp belongs to.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Control {
    Pause,
    Warp,
    Mute,
    /// The keys page: not a toggle of the game's but of the panel's, and
    /// its lamp says the page is showing.
    Keys,
}

/// **Which half the panel shows under its icon row**: the reading — the
/// tally and the new-run bar — or the keys. A component on each half's
/// root, which shows exactly when it is the menu's page.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    #[default]
    Reading,
    Keys,
}

/// The name printed on an action's keycap.
#[derive(Component, Clone, Copy)]
struct CapText(Action);

/// A run of the refusal slash over an action's keycap, shown while a key
/// it may not take was just pressed into it.
#[derive(Component, Clone, Copy)]
struct CapSlash(Action);

/// The scrim: the whole screen behind the panel. Clicking it is "put it
/// away", the same answer `Esc` gives.
#[derive(Component)]
struct Scrim;

/// The panel body — a click blocker, so the bare metal of the menu is
/// not a dismiss target.
#[derive(Component)]
struct Panel;

/// The menu's root, shown and hidden as one.
#[derive(Component)]
struct MenuRoot;

/// What a node in the menu is, for the one pass that repaints them all.
/// One component rather than a marker per kind, on purpose: every part
/// of the menu is a colored rectangle answering to the same frame of sim
/// state, so one query paints the lot and nothing can drift out of step.
#[derive(Component, Clone, Copy)]
enum Paint {
    /// A control's clickable face.
    Face(Control),
    /// One run of an icon glyph.
    Ink(Control),
    /// The state lamp under a control's glyph.
    Lamp(Control),
    /// One rung of the delivery tally, by ladder index.
    Pip(usize),
    /// The new-run bar's clickable face. Not a [`Control`]: a control
    /// toggles and wears a lamp for its state, and this does neither.
    NewRun,
    /// An action's keycap on the keys page.
    Keycap(Action),
    /// One run of an action's glyph on the keys page.
    Mark(Action),
    /// The keycap that puts every key back on its default.
    Defaults,
}

/// A piece of the mark that says the world is not running as it should:
/// the full-screen node that hangs it, the box it is stamped in, and
/// every run of the glyph. All three wear one, because the closed-menu
/// law has to tell the panel from the reading and ask each the question
/// that belongs to it.
#[derive(Component, Clone, Copy)]
struct Telltale(Control);

/// The mark's own root, whose visibility governs the whole of it.
#[derive(Component)]
struct MarkRoot;

/// A run of the mute slash, shown only while muted. Not a [`Paint`]: its
/// color never changes, only whether the refusal is there at all.
#[derive(Component, Clone)]
struct Slash;

/// The controls worked this frame, drained by `advance` into the
/// `InputFrame`. Edges, not states: the sim owns every state here.
// One edge per control is honestly a pile of booleans, same as
// `FrameInput`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Worked {
    pub pause: bool,
    pub warp: bool,
    pub mute: bool,
    pub reseed: bool,
}

/// The menu: whether it stands, what was worked on it this frame, which
/// page it shows, and the keycap waiting for a key.
#[derive(Resource, Default)]
pub struct Menu {
    pub open: bool,
    worked: Worked,
    page: Page,
    /// The action whose keycap waits for a key, if one does.
    capture: Option<Action>,
    /// Seconds left on the slash a waiting keycap wears for a key it was
    /// refused.
    refused: f32,
}

impl Menu {
    /// Boot state. `--menu` opens it standing, for screenshot runs, and
    /// `--menu keys` on the keys page.
    #[must_use]
    pub const fn boot(open: bool) -> Self {
        Self {
            open,
            worked: Worked {
                pause: false,
                warp: false,
                mute: false,
                reseed: false,
            },
            page: Page::Reading,
            capture: None,
            refused: 0.0,
        }
    }

    /// The same menu, on `page`.
    #[must_use]
    pub const fn on(mut self, page: Page) -> Self {
        self.page = page;
        self
    }

    /// Drain this frame's control edges. Exactly one consumer
    /// (`advance`), exactly once per frame — same law the sim's own
    /// pointer edges keep.
    pub const fn take(&mut self) -> Worked {
        let worked = self.worked;
        self.worked = Worked {
            pause: false,
            warp: false,
            mute: false,
            reseed: false,
        };
        worked
    }

    /// Put the menu away and hand the cursor back to the room. It opens
    /// again on its reading, with no keycap waiting.
    const fn close(&mut self, rig: &mut CameraRig) {
        self.open = false;
        rig.parked = false;
        self.page = Page::Reading;
        self.stop_waiting();
    }

    /// No keycap waits any more, and no refusal is showing.
    const fn stop_waiting(&mut self) {
        self.capture = None;
        self.refused = 0.0;
    }
}

/// The menu's plugin: build it hidden after the rig stands, then read
/// the sim back onto it every view frame.
pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, (spawn, mark))
            .add_systems(Update, click.in_set(Phase::Input).after(keys))
            .add_systems(Update, (paint, show_mark).in_set(Phase::View));
    }
}

// ------------------------------------------------------------------ spawn --

/// Spawn one glyph's runs as absolutely-placed cells inside `holder`,
/// tagging each with whatever marker the caller wants on it.
fn stamp<M: Component + Clone>(
    commands: &mut Commands,
    holder: Entity,
    glyph: &Glyph,
    color: Color,
    marker: M,
) {
    for (y, row) in glyph.rows.iter().enumerate() {
        let mut x = 0_u8;
        while x < glyph.w {
            let lit = |x: u8| row & (1 << (glyph.w - 1 - x)) != 0;
            if !lit(x) {
                x += 1;
                continue;
            }
            let start = x;
            while x < glyph.w && lit(x) {
                x += 1;
            }
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(f32::from(start) * CELL),
                    top: px(y as f32 * CELL),
                    width: px(f32::from(x - start) * CELL),
                    height: px(CELL),
                    ..default()
                },
                BackgroundColor(color),
                Pickable::IGNORE,
                marker.clone(),
                ChildOf(holder),
            ));
        }
    }
}

/// One control button: a face, a stamped icon, a state lamp.
fn button(commands: &mut Commands, row: Entity, control: Control, glyph: &Glyph) {
    let face = commands
        .spawn((
            Button,
            Node {
                width: px(FACE),
                height: px(FACE),
                border: UiRect::all(px(2)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(5),
                ..default()
            },
            BackgroundColor(palette::PLATE),
            BorderColor::all(palette::PLATE_SHADE),
            Paint::Face(control),
            ChildOf(row),
        ))
        .id();
    let holder = commands
        .spawn((
            Node {
                width: px(f32::from(glyph.w) * CELL),
                height: px(glyph.rows.len() as f32 * CELL),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(face),
        ))
        .id();
    stamp(commands, holder, glyph, palette::ICON, Paint::Ink(control));
    if control == Control::Mute {
        // Stamped after the speaker so it lies over it, hidden until the
        // sound actually stops.
        stamp(commands, holder, &SLASH, palette::LAMP_NO, Slash);
    }
    commands.spawn((
        Node {
            width: px(LAMP_W),
            height: px(LAMP_H),
            ..default()
        },
        BackgroundColor(palette::GLASS),
        Pickable::IGNORE,
        Paint::Lamp(control),
        ChildOf(face),
    ));
}

/// The new-run bar: the one control that ends something, so the one that
/// says what it does in words. It stands at the panel's foot, below the
/// tally: out of the icon row, and clear of the screen centre, where the
/// crosshair was a moment ago and where a freed cursor is most likely to
/// be when the menu opens. It is a socket-dark well in a red frame, as
/// wide as the panel, so it never reads as one more plate.
fn new_run(commands: &mut Commands, panel: Entity) {
    let bar = commands
        .spawn((
            Button,
            Node {
                width: percent(100),
                height: px(NEW_RUN_H),
                margin: UiRect::top(px(NEW_RUN_GAP)),
                border: UiRect::all(px(2)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::SOCKET),
            BorderColor::all(palette::LAMP_NO),
            Paint::NewRun,
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new(NEW_RUN),
        TextFont {
            font_size: FontSize::Px(NEW_RUN_TEXT),
            ..default()
        },
        TextColor(palette::GLINT),
        Pickable::IGNORE,
        ChildOf(bar),
    ));
}

/// Build the whole menu, hidden. Warp is dev-only furniture here for the
/// same reason it was on the wall: the 16× fast-forward is a developer's
/// key, and a control the player cannot use is a control that should not
/// be drawn.
fn spawn(mut commands: Commands, shell: Res<Shell>, bindings: Res<Bindings>) {
    let root = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::VOID.with_alpha(0.55)),
            Visibility::Hidden,
            GlobalZIndex(3),
            Scrim,
            MenuRoot,
        ))
        .id();
    let panel = commands
        .spawn((
            Button,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                padding: UiRect::all(px(18)),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(palette::HULL.with_alpha(0.94)),
            BorderColor::all(palette::PLATE_LIT),
            Panel,
            ChildOf(root),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(12),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    button(&mut commands, row, Control::Pause, &PAUSE);
    if shell.bridge.dev() {
        button(&mut commands, row, Control::Warp, &FAST);
    }
    button(&mut commands, row, Control::Mute, &SPEAKER);
    button(&mut commands, row, Control::Keys, &KEYBOARD);

    // The panel's lower half: the reading, or the keys. Both are built
    // and one is laid out at a time (`paint`), so the panel is as tall as
    // whichever half it shows.
    let reading = page(&mut commands, panel, Page::Reading);
    hairline(&mut commands, reading);

    // The tally: the hangar strip's own ladder, off the wall and onto
    // the overlay. Six rungs, violet, because the count is the Guild's
    // business and not the ship's status.
    let tally = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(PIP_GAP),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(reading),
        ))
        .id();
    for i in 0..DELIVERY_LAMPS.len() {
        commands.spawn((
            Node {
                width: px(PIP),
                height: px(PIP),
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(palette::GLASS),
            BorderColor::all(palette::SOCKET),
            Pickable::IGNORE,
            Paint::Pip(i),
            ChildOf(tally),
        ));
    }

    new_run(&mut commands, reading);

    let keys = page(&mut commands, panel, Page::Keys);
    hairline(&mut commands, keys);
    keys_page(&mut commands, keys, &bindings);
}

/// One half of the panel under its icon row, laid out only while it is
/// the menu's page.
fn page(commands: &mut Commands, panel: Entity, page: Page) -> Entity {
    commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(14),
                display: Display::None,
                ..default()
            },
            Pickable::IGNORE,
            page,
            ChildOf(panel),
        ))
        .id()
}

/// A hairline between the controls and what is under them: one is
/// something you do, the other is something that happened, or the keys
/// you do it with.
fn hairline(commands: &mut Commands, holder: Entity) {
    commands.spawn((
        Node {
            width: percent(100),
            height: px(2),
            ..default()
        },
        BackgroundColor(palette::PLATE_SHADE),
        Pickable::IGNORE,
        ChildOf(holder),
    ));
}

/// **The keys page**: one row per carry action — its glyph, stamped the
/// way the icon row's are, and its keycap — and the keycap that puts them
/// all back. A table of actions and not four fixed rows, so the next
/// thing a key does is one more entry in `keys::Action::ALL`.
fn keys_page(commands: &mut Commands, page: Entity, bindings: &Bindings) {
    let table = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(ROW_GAP),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(page),
        ))
        .id();
    // Every glyph stands centred in a column as wide as the widest, so
    // the keycaps stand in one column whatever is drawn beside them.
    let widest = Action::ALL
        .iter()
        .map(|action| glyph_of(*action).w)
        .max()
        .unwrap_or(0);
    for action in Action::ALL {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(ROW_SPLIT),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(table),
            ))
            .id();
        let glyph = glyph_of(action);
        let column = commands
            .spawn((
                Node {
                    width: px(f32::from(widest) * CELL),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(row),
            ))
            .id();
        let holder = commands
            .spawn((
                Node {
                    width: px(f32::from(glyph.w) * CELL),
                    height: px(glyph.rows.len() as f32 * CELL),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(column),
            ))
            .id();
        stamp(
            commands,
            holder,
            glyph,
            palette::ICON_LIT,
            Paint::Mark(action),
        );
        let cap = keycap(commands, row, CAP_W, Paint::Keycap(action));
        commands.spawn((
            keycap_text(crate::keys::name(bindings.key(action)).unwrap_or_default()),
            CapText(action),
            ChildOf(cap),
        ));
        // The refusal, stamped over the cap and hidden until a key the
        // game keeps is pressed into it.
        // Centred on the cap's face by its own measure: an absolute node
        // is placed from the face's top left, inside its border.
        let (w, h) = (f32::from(SLASH.w) * CELL, SLASH.rows.len() as f32 * CELL);
        let over = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px((CAP_W - 4.0 - w) * 0.5),
                    top: px((CAP_H - 2.0 - CAP_LIP - h) * 0.5),
                    width: px(w),
                    height: px(h),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(cap),
            ))
            .id();
        stamp(commands, over, &SLASH, palette::LAMP_NO, CapSlash(action));
    }
    let defaults = keycap(commands, page, CAP_W * 2.0, Paint::Defaults);
    commands.spawn((keycap_text(DEFAULTS), ChildOf(defaults)));
}

/// **A keycap**: a plate standing on a thicker bottom edge than its other
/// three, the way a key on a keyboard stands, so a player reads it as a
/// key before reading what it says.
fn keycap(commands: &mut Commands, holder: Entity, width: f32, paint: Paint) -> Entity {
    commands
        .spawn((
            Button,
            Node {
                width: px(width),
                height: px(CAP_H),
                border: UiRect {
                    left: px(2),
                    right: px(2),
                    top: px(2),
                    bottom: px(CAP_LIP),
                },
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::PLATE),
            BorderColor::all(palette::PLATE_SHADE),
            paint,
            ChildOf(holder),
        ))
        .id()
}

/// **What a keycap prints**: a key's name, or the one word under the
/// table. The keys page's only text, and DESIGN.md's second sanctioned
/// exception to the law against it: a key's name is text by nature.
fn keycap_text(name: &str) -> impl Bundle + use<> {
    (
        Text::new(name),
        TextFont {
            font_size: FontSize::Px(CAP_TEXT),
            ..default()
        },
        TextColor(palette::GLINT),
        Pickable::IGNORE,
    )
}

/// **Hang the two marks, hidden.** They are not the panel and they are
/// not the room: the panel is a thing you work and the room is a thing
/// the sim describes, and this is neither — it is the game saying what
/// it is doing with the world while nobody has the panel open.
///
/// Under the crosshair rather than in a corner, because the press that
/// goes nowhere is an aimed press and the eye is already there when it
/// fails. Fast-forward is dev furniture here for the same reason its
/// button is: `Bridge` refuses a warp toggle outside a dev run, so
/// outside one the state cannot happen and the mark for it would be a
/// reading of nothing.
fn mark(mut commands: Commands, shell: Res<Shell>) {
    let mut hang = |control: Control, glyph: &Glyph| {
        let root = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Pickable::IGNORE,
                Visibility::Hidden,
                // The crosshair's own plane: this hangs off it, and the
                // panel (3) still covers both when it stands.
                GlobalZIndex(1),
                Telltale(control),
                MarkRoot,
            ))
            .id();
        let holder = commands
            .spawn((
                Node {
                    top: px(TELL_DROP),
                    width: px(f32::from(glyph.w) * CELL),
                    height: px(glyph.rows.len() as f32 * CELL),
                    ..default()
                },
                Pickable::IGNORE,
                Telltale(control),
                ChildOf(root),
            ))
            .id();
        stamp(
            &mut commands,
            holder,
            glyph,
            palette::AMBER,
            Telltale(control),
        );
    };
    hang(Control::Pause, &PAUSE);
    if shell.bridge.dev() {
        hang(Control::Warp, &FAST);
    }
}

// ------------------------------------------------------------------- input --

/// `Esc`, and what it means where; and a waiting keycap's key. Runs first
/// in `Phase::Input`, ahead of the camera: an `Esc` this answers is one
/// `rig::steer` must never also act on, and `steer` returns early while
/// the menu stands.
///
/// **A waiting keycap takes the next key, and nothing else does.** The
/// key is bound ([`Bindings::bind`], which swaps it with the action that
/// held it) and the cap stops waiting; a key the game keeps is refused,
/// and the cap goes on waiting with its slash up. Either way the press is
/// the cap's: it is taken off the frame's input here, so a refused
/// `Space` pauses nothing and a bound `F` warps nothing. `Esc` stops the
/// waiting and leaves the menu up — the nearest thing it can mean.
pub fn keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut bindings: ResMut<Bindings>,
    mut rig: ResMut<CameraRig>,
    mut menu: ResMut<Menu>,
    time: Res<Time>,
) {
    menu.refused = (menu.refused - time.delta_secs()).max(0.0);
    if let Some(action) = menu.capture.filter(|_| menu.open) {
        if keys.just_pressed(KeyCode::Escape) {
            keys.clear_just_pressed(KeyCode::Escape);
            menu.stop_waiting();
        } else if let Some(key) = keys
            .get_just_pressed()
            .copied()
            .min_by_key(|key| format!("{key:?}"))
        {
            keys.clear_just_pressed(key);
            match bindings.bind(action, key) {
                Ok(_) => menu.stop_waiting(),
                Err(_) => menu.refused = REFUSAL_LEN,
            }
        }
    }
    if keys.just_pressed(KeyCode::Escape) {
        if menu.open {
            menu.close(&mut rig);
        } else if matches!(rig.mode, Mode::Roam) {
            // Roaming: the menu opens and the cursor goes free — the
            // same parking `Esc` has always done, with something to
            // click. Focused or mid-glide, this is not ours: `steer`
            // steps out of the station first, and the next `Esc` (now
            // roaming) opens the menu.
            menu.open = true;
        }
    }
    // The standing invariant: an open menu keeps the cursor free. Said
    // every frame rather than once, so a `--menu` boot is honest too.
    if menu.open {
        rig.parked = true;
    }
}

/// Clicks on the faces, the new-run bar, the keys page and the scrim. A
/// face or the bar throws its edge; the keyboard face turns the page; a
/// keycap starts waiting for a key, and the defaults cap puts every key
/// back; the bare scrim dismisses.
fn click(
    mut menu: ResMut<Menu>,
    mut rig: ResMut<CameraRig>,
    mut bindings: ResMut<Bindings>,
    faces: Query<(&Interaction, &Paint), Changed<Interaction>>,
    scrim: Query<&Interaction, (With<Scrim>, Changed<Interaction>)>,
) {
    if !menu.open {
        return;
    }
    for (interaction, paint) in &faces {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match paint {
            Paint::Face(Control::Pause) => menu.worked.pause = true,
            Paint::Face(Control::Warp) => menu.worked.warp = true,
            Paint::Face(Control::Mute) => menu.worked.mute = true,
            Paint::Face(Control::Keys) => {
                menu.page = match menu.page {
                    Page::Reading => Page::Keys,
                    Page::Keys => Page::Reading,
                };
                menu.stop_waiting();
            }
            Paint::NewRun => menu.worked.reseed = true,
            Paint::Keycap(action) => {
                menu.stop_waiting();
                menu.capture = Some(*action);
            }
            Paint::Defaults => {
                bindings.reset();
                menu.stop_waiting();
            }
            Paint::Ink(_) | Paint::Lamp(_) | Paint::Pip(_) | Paint::Mark(_) => {}
        }
    }
    if scrim
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        menu.close(&mut rig);
    }
}

// -------------------------------------------------------------------- view --

/// The nodes in the menu that come and go: the root (the whole overlay),
/// the mute slash, and the refusal slash over each keycap. They share a
/// query because two `&mut Visibility` params in one system are a
/// conflict Bevy refuses at startup — and `Has` and an `Option` answer
/// which is which more cheaply than proving three filters disjoint.
type Shown<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Visibility,
        Has<MenuRoot>,
        Option<&'static CapSlash>,
    ),
    Or<(With<MenuRoot>, With<Slash>, With<CapSlash>)>,
>;

/// Every coloured rectangle the menu repaints, with the border a keycap
/// lights to say it waits.
type Parts<'w, 's> = Query<
    'w,
    's,
    (
        &'static Paint,
        Option<&'static Interaction>,
        &'static mut BackgroundColor,
        Option<&'static mut BorderColor>,
    ),
>;

/// The flat equivalent of `glow::set_lamp`: glass at zero, the lamp's
/// own color at one. No emissive out here — the menu is not in the room,
/// so it gets no bloom and wants none.
fn lamp_color(color: Color, level: f32) -> Color {
    palette::mix(palette::GLASS, color, level.clamp(0.0, 1.0))
}

/// Read the sim back onto the menu: icon inks, state lamps, the mute
/// slash, the tally, the page, and the keys page's caps. Everything it
/// shows, it shows because the sim, the bindings or the menu say so this
/// frame — nothing here caches a toggle of its own.
#[allow(clippy::too_many_lines)]
fn paint(
    shell: Res<Shell>,
    menu: Res<Menu>,
    bindings: Res<Bindings>,
    mut shown: Shown,
    mut parts: Parts,
    mut pages: Query<(&Page, &mut Node)>,
    mut names: Query<(&CapText, &mut Text)>,
) {
    let muted = shell.muted;
    let waiting = menu.capture.filter(|_| menu.open);
    for (mut visibility, is_root, slash) in &mut shown {
        // The root stands or does not; the slash rides mute. They hide
        // by the same word but they show by different ones, and the
        // difference is the whole of it: `Visible` means *drawn even
        // though an ancestor is hidden*, which is the right answer for
        // an overlay that answers to nothing and the wrong one for a
        // mark inside it. Said of the slash it put a red bar across the
        // played frame every time a player muted and shut the menu.
        // Under the root, showing is `Inherited`: the slash says mute is
        // on, the root says whether anyone is being told. A keycap's
        // slash says the same of a refused key.
        let up = match slash {
            Some(CapSlash(action)) => waiting == Some(*action) && menu.refused > 0.0,
            None if is_root => menu.open,
            None => muted,
        };
        *visibility = match (up, is_root) {
            (true, true) => Visibility::Visible,
            (true, false) => Visibility::Inherited,
            (false, _) => Visibility::Hidden,
        };
    }
    if !menu.open {
        return;
    }
    // One half of the panel is laid out at a time, so the panel is as
    // tall as the half it shows.
    for (page, mut node) in &mut pages {
        let display = if *page == menu.page {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    // A keycap prints its key's name, and a waiting one prints nothing:
    // the blank amber cap is the question.
    for (CapText(action), mut text) in &mut names {
        let name = if waiting == Some(*action) {
            ""
        } else {
            crate::keys::name(bindings.key(*action)).unwrap_or_default()
        };
        if text.0 != name {
            name.clone_into(&mut text.0);
        }
    }
    let sim = &shell.bridge.sim;
    let paused = sim.is_paused();
    let warping = sim.is_warp();
    let deliveries = sim.deliveries();
    let live = |control: Control| match control {
        Control::Pause => paused,
        Control::Warp => warping,
        // The speaker's icon reads *sound*, not *mute*: lit while the
        // ship can be heard, and the slash says the rest.
        Control::Mute => !muted,
        // The keyboard is lit while its page is the one showing.
        Control::Keys => menu.page == Page::Keys,
    };
    // Which face the cursor rests on, spent below on that control's
    // lamp — so the tell reaches the whole button, not just its border.
    let hovered = parts
        .iter()
        .find_map(|(paint, interaction, _, _)| match (paint, interaction) {
            (Paint::Face(control), Some(Interaction::Hovered | Interaction::Pressed)) => {
                Some(*control)
            }
            _ => None,
        });
    let under = |interaction: Option<&Interaction>| {
        matches!(
            interaction,
            Some(Interaction::Hovered | Interaction::Pressed)
        )
    };

    for (paint, interaction, mut background, border) in &mut parts {
        background.0 = match paint {
            Paint::Face(_) => {
                if under(interaction) {
                    palette::PLATE_LIT
                } else {
                    palette::PLATE
                }
            }
            // The defaults cap sits sunk to the socket while there is
            // nothing to put back, and stands up as a key once there is.
            Paint::Defaults => {
                if under(interaction) {
                    palette::PLATE_LIT
                } else if bindings.defaults() {
                    palette::SOCKET
                } else {
                    palette::PLATE
                }
            }
            // A live function wears its lamp's own color; a sleeping one
            // stays etched metal.
            Paint::Ink(control) => match control {
                Control::Pause | Control::Warp | Control::Keys => {
                    if live(*control) {
                        palette::AMBER
                    } else {
                        palette::ICON
                    }
                }
                Control::Mute => {
                    if muted {
                        palette::ICON
                    } else {
                        palette::ICON_LIT
                    }
                }
            },
            Paint::Lamp(control) => {
                let (color, level) = match control {
                    Control::Pause | Control::Warp | Control::Keys => {
                        (palette::AMBER, if live(*control) { 1.0 } else { 0.0_f32 })
                    }
                    Control::Mute => (palette::LAMP_OK, if muted { 0.0 } else { SPEAKER_LEVEL }),
                };
                // Hover wakes a sleeping lamp faintly — interactable,
                // not active. The console face's own courtesy, kept.
                let level = if hovered == Some(*control) {
                    level.max(HOVER_WAKE)
                } else {
                    level
                };
                lamp_color(color, level)
            }
            Paint::Pip(i) => {
                if deliveries >= DELIVERY_LAMPS[*i] {
                    palette::EERIE
                } else {
                    palette::GLASS
                }
            }
            // The well rises to plate under the cursor: the same answer
            // a face gives, one step lower, because it starts lower.
            Paint::NewRun => {
                if under(interaction) {
                    palette::PLATE
                } else {
                    palette::SOCKET
                }
            }
            // A waiting keycap is lifted and framed in amber — a live
            // function, waiting on the hand — and framed in the refusal
            // red, slash and all, for a moment after a key it may not
            // take.
            Paint::Keycap(action) => {
                if let Some(mut border) = border {
                    *border = BorderColor::all(match waiting {
                        Some(at) if at == *action && menu.refused > 0.0 => palette::LAMP_NO,
                        Some(at) if at == *action => palette::AMBER,
                        _ => palette::PLATE_SHADE,
                    });
                }
                if waiting == Some(*action) || under(interaction) {
                    palette::PLATE_LIT
                } else {
                    palette::PLATE
                }
            }
            // The action whose cap waits is the live one.
            Paint::Mark(action) => {
                if waiting == Some(*action) {
                    palette::AMBER
                } else {
                    palette::ICON_LIT
                }
            }
        };
    }
}

/// Hang whichever mark the world has earned this frame, panel open or
/// shut. A separate pass from `paint` on purpose: `paint` returns the
/// moment the menu is down, and being down is exactly when this has
/// something to say.
///
/// Paused wins over warp when the sim reports both, because a world
/// that is not advancing is not advancing fast — and the sim itself
/// takes the same view, ignoring warp entirely while pause stands.
fn show_mark(shell: Res<Shell>, mut marks: Query<(&Telltale, &mut Visibility), With<MarkRoot>>) {
    let sim = &shell.bridge.sim;
    let wanted = if sim.is_paused() {
        Some(Control::Pause)
    } else if sim.is_warp() {
        Some(Control::Warp)
    } else {
        None
    };
    for (telltale, mut visibility) in &mut marks {
        // `Visible` here means what it says and nothing more: a mark's
        // root answers to no ancestor, so it is the plain word for up.
        *visibility = if wanted == Some(telltale.0) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use space_trucking::sim::{Cue, InputFrame};

    use super::*;
    use crate::bridge::{Bridge, FrameOutcome};
    use crate::rig::Focus;

    /// The edges drain exactly once: `advance` is the only consumer, and
    /// a toggle that survived its frame would fire the sim twice.
    #[test]
    fn worked_edges_drain_once() {
        let mut menu = Menu::boot(false);
        menu.worked.pause = true;
        menu.worked.mute = true;
        menu.worked.reseed = true;
        let first = menu.take();
        assert!(first.pause && first.mute && first.reseed && !first.warp);
        let second = menu.take();
        assert!(!second.pause && !second.mute && !second.reseed && !second.warp);
    }

    /// A bare app with the menu's input systems and nothing else — no
    /// window, no renderer. The whole `Esc` contract is drivable this
    /// way because it is only ever three things talking: the key state,
    /// the menu, and the camera rig.
    fn harness(open: bool, mode: Mode) -> App {
        let mut app = App::new();
        let mut rig = CameraRig::boot(None);
        rig.mode = mode;
        app.insert_resource(Menu::boot(open))
            .insert_resource(rig)
            .insert_resource(Bindings::default())
            .init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (keys, click).chain());
        app
    }

    /// Press `key`, run one frame, and let it back up.
    fn tap(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(key);
        keys.clear();
    }

    /// Click the keycap of `action`, as `click` hears a press on it.
    fn press_cap(app: &mut App, paint: Paint) {
        let cap = app.world_mut().spawn((Interaction::Pressed, paint)).id();
        app.update();
        app.world_mut().entity_mut(cap).despawn();
    }

    /// Press `Esc`, run one frame, and let the key back up — a key held
    /// down never presses twice, which is the whole reason this is a
    /// helper and not two lines inlined.
    fn escape(app: &mut App) {
        tap(app, KeyCode::Escape);
    }

    /// **The `Esc` contract, whole.** Roaming, `Esc` raises the menu and
    /// frees the cursor — the parking it always did. At a station it is
    /// not ours at all: `rig::steer` steps out of the focus first, and
    /// only the next `Esc`, now roaming, opens anything. And `Esc` on an
    /// open menu puts it away and takes the cursor back.
    #[test]
    fn escape_steps_out_before_it_opens_anything() {
        let mut app = harness(false, Mode::Roam);
        escape(&mut app);
        assert!(app.world().resource::<Menu>().open, "roam Esc opens it");
        assert!(
            app.world().resource::<CameraRig>().parked,
            "an open menu must free the cursor, exactly as parking did"
        );
        escape(&mut app);
        assert!(!app.world().resource::<Menu>().open, "Esc puts it away");
        assert!(
            !app.world().resource::<CameraRig>().parked,
            "closing hands the cursor back to the room"
        );

        // Focused: the station's own Esc. The menu keeps its hands off.
        let mut app = harness(false, Mode::Focused { focus: Focus::Tank });
        escape(&mut app);
        assert!(
            !app.world().resource::<Menu>().open,
            "Esc at a station belongs to the camera, not the menu"
        );
    }

    /// A press on a control face, or on the new-run bar, throws exactly
    /// that control's edge, and only while the menu stands: the sim must
    /// never hear from a menu that is not on screen.
    #[test]
    fn a_pressed_face_throws_its_own_edge() {
        let mut app = harness(true, Mode::Roam);
        app.world_mut()
            .spawn((Interaction::Pressed, Paint::Face(Control::Mute)));
        app.update();
        let worked = app.world_mut().resource_mut::<Menu>().take();
        assert!(worked.mute && !worked.pause && !worked.warp && !worked.reseed);

        let mut app = harness(true, Mode::Roam);
        app.world_mut().spawn((Interaction::Pressed, Paint::NewRun));
        app.update();
        let worked = app.world_mut().resource_mut::<Menu>().take();
        assert!(worked.reseed && !worked.pause && !worked.warp && !worked.mute);

        let mut app = harness(false, Mode::Roam);
        app.world_mut()
            .spawn((Interaction::Pressed, Paint::Face(Control::Pause)));
        app.world_mut().spawn((Interaction::Pressed, Paint::NewRun));
        app.update();
        let worked = app.world_mut().resource_mut::<Menu>().take();
        assert!(
            !worked.pause && !worked.reseed,
            "a closed menu threw an edge at the sim anyway"
        );
    }

    /// The menu's input systems with the real `advance` behind them, over
    /// the developer fixture. The clock stands still and the pointer
    /// rests on nothing, so an edge the menu throws is the only thing
    /// that can change the world between two frames.
    fn played() -> App {
        let mut bridge = Bridge::boot_fixture(crate::fixture::SAVE);
        bridge.steady();
        let mut app = App::new();
        app.insert_resource(Shell {
            bridge,
            outcome: FrameOutcome::default(),
            muted: false,
        })
        .insert_resource(Menu::boot(false))
        .insert_resource(CameraRig::boot(None))
        .insert_resource(Bindings::default())
        .init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
        .init_resource::<crate::surface::VirtualPointer>()
        .init_resource::<crate::gesture::Grips>()
        .init_resource::<crate::room::Occupancy>()
        .init_resource::<crate::room::AimedLatch>()
        .add_systems(Update, (keys, click, crate::advance).chain());
        app
    }

    /// **The new-run bar starts a new run, by the road the key took.**
    /// `click` sets the edge, `advance` drains it into the frame, and the
    /// bridge hands the sim a fresh seed: the same fold the `R` key fed
    /// before it was cut. What comes back is a replacement and not an
    /// advance, so the tick counter reads nought. The key's half of the
    /// law, that it no longer does this, is asked of the whole input
    /// schedule in `main`'s session (`r_is_only_a_letter_now`).
    #[test]
    fn the_new_run_bar_starts_a_new_run() {
        let mut app = played();
        let seed = app.world().resource::<Shell>().bridge.sim.seed();
        escape(&mut app);
        assert!(app.world().resource::<Menu>().open, "roam Esc opens it");
        assert_eq!(
            app.world().resource::<Shell>().bridge.sim.seed(),
            seed,
            "opening the menu replaced the world"
        );

        app.world_mut().spawn((Interaction::Pressed, Paint::NewRun));
        app.update();
        let sim = &app.world().resource::<Shell>().bridge.sim;
        assert!(
            sim.cues().contains(&Cue::Reseed),
            "the bar was pressed and the sim never heard: {:?}",
            sim.cues()
        );
        assert_ne!(sim.seed(), seed, "a new run kept the old seed");
        assert_eq!(sim.tick(), 0, "a new run advanced instead of replacing");
    }

    /// A press on the bare scrim — the room showing through around the
    /// panel — dismisses, same as `Esc`.
    #[test]
    fn the_scrim_dismisses() {
        let mut app = harness(true, Mode::Roam);
        app.world_mut().spawn((Interaction::Pressed, Scrim));
        app.update();
        assert!(!app.world().resource::<Menu>().open);
        assert!(!app.world().resource::<CameraRig>().parked);
    }

    /// Every glyph fits the width it declares — a stray bit past the
    /// left edge would silently draw a cell nobody authored.
    #[test]
    fn every_glyph_stays_inside_its_width() {
        for (name, glyph) in [
            ("pause", &PAUSE),
            ("fast", &FAST),
            ("speaker", &SPEAKER),
            ("slash", &SLASH),
            ("keyboard", &KEYBOARD),
            ("turn ccw", &TURN_CCW),
            ("turn cw", &TURN_CW),
            ("raise", &RAISE),
            ("lower", &LOWER),
        ] {
            for (y, row) in glyph.rows.iter().enumerate() {
                assert!(
                    *row < (1_u16 << glyph.w),
                    "{name} row {y} sets a bit outside its {} cells",
                    glyph.w
                );
            }
            assert!(!glyph.rows.is_empty(), "{name} draws nothing");
        }
    }

    /// The menu renders two kinds of string: the new-run label, and what
    /// a keycap prints. The law is DESIGN.md's ("absolutely no text"). The
    /// version corner, this label and the keycaps are its exemptions, and
    /// this file is exactly the kind of place that would quietly grow
    /// another — a menu is what menus are usually made of words for. So a
    /// rendered string is made in two places and no third: the new-run
    /// bar, and the one function every keycap prints through, which is
    /// handed a key's name or the one word under the table and nothing
    /// else.
    #[test]
    fn the_menu_speaks_in_shapes_but_for_its_label_and_its_keycaps() {
        let source = include_str!("menu.rs");
        // Spelled in pieces so the test does not trip over itself.
        let text_node = ["Text", "::new"].concat();
        assert_eq!(
            source.matches(&text_node).count(),
            2,
            "the menu grew a rendered string; icons only, the new-run label and the keycaps \
             excepted (DESIGN.md)"
        );
        assert!(
            source.contains(&[text_node.as_str(), "(NEW_RUN)"].concat()),
            "the menu's one sentence is not the new-run label"
        );
        assert!(
            source.contains(&[text_node.as_str(), "(name)"].concat()),
            "the menu's other string is not a keycap's"
        );
        let printed = ["keycap", "_text("].concat();
        let words: Vec<&str> = source
            .split(&printed)
            .skip(1)
            .filter_map(|call| call.split_once(')').map(|(argument, _)| argument))
            .filter(|argument| *argument != "name: &str")
            .collect();
        assert_eq!(
            words.len(),
            2,
            "a keycap printed something other than a key's name or the defaults word: {words:?}"
        );
        assert!(
            words
                .iter()
                .any(|word| word.starts_with("crate::keys::name("))
                && words.contains(&"DEFAULTS"),
            "a keycap printed something other than a key's name or the defaults word: {words:?}"
        );
    }

    /// **Click a keycap, press a key, done** (docs/BAY.md, "Lift, and the
    /// keys"). The cap waits for the next key, binds it, and stops
    /// waiting. A key another action holds swaps the two. And the key
    /// that answered the cap is the cap's alone: it is gone from the
    /// frame's input before anything else in the game reads it.
    #[test]
    fn a_keycap_takes_the_next_key_and_a_held_key_swaps() {
        let mut app = harness(true, Mode::Roam);
        press_cap(&mut app, Paint::Keycap(Action::Raise));
        assert_eq!(
            app.world().resource::<Menu>().capture,
            Some(Action::Raise),
            "a clicked keycap did not wait"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyC),
            "the key that answered the cap was left for the game to read"
        );
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::KeyC);
        keys.clear();
        assert_eq!(
            app.world().resource::<Bindings>().key(Action::Raise),
            KeyCode::KeyC
        );
        assert_eq!(
            app.world().resource::<Menu>().capture,
            None,
            "the cap kept waiting"
        );

        press_cap(&mut app, Paint::Keycap(Action::Lower));
        tap(&mut app, KeyCode::KeyQ);
        let bindings = app.world().resource::<Bindings>();
        assert_eq!(bindings.key(Action::Lower), KeyCode::KeyQ);
        assert_eq!(
            bindings.key(Action::TurnCcw),
            KeyCode::KeyZ,
            "binding a held key left its old action unbound"
        );

        press_cap(&mut app, Paint::Defaults);
        assert!(
            app.world().resource::<Bindings>().defaults(),
            "the defaults cap did nothing"
        );
    }

    /// **`Esc` stops a waiting keycap and leaves the menu standing**; the
    /// next `Esc` puts the menu away as it always did.
    #[test]
    fn escape_stops_a_waiting_keycap_without_closing_the_menu() {
        let mut app = harness(true, Mode::Roam);
        press_cap(&mut app, Paint::Keycap(Action::TurnCw));
        escape(&mut app);
        let menu = app.world().resource::<Menu>();
        assert_eq!(menu.capture, None, "Esc left the cap waiting");
        assert!(menu.open, "Esc on a waiting cap closed the menu");
        assert!(
            app.world().resource::<Bindings>().defaults(),
            "Esc was bound to a carry action"
        );
        escape(&mut app);
        assert!(
            !app.world().resource::<Menu>().open,
            "the next Esc kept the menu up"
        );
    }

    /// **A key the game keeps is refused**, and the cap goes on waiting
    /// with the refusal on it: the walk, a toggle, a modifier, and the
    /// bench's own keys. The refused key is still the cap's, so a `Space`
    /// pressed into it pauses nothing; and a key the page may bind binds
    /// after it.
    #[test]
    fn a_reserved_key_is_refused_and_the_cap_waits_on() {
        let mut app = harness(true, Mode::Roam);
        press_cap(&mut app, Paint::Keycap(Action::Raise));
        for key in [
            KeyCode::KeyW,
            KeyCode::Space,
            KeyCode::ShiftLeft,
            KeyCode::KeyR,
            KeyCode::ArrowUp,
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            assert!(
                !app.world()
                    .resource::<ButtonInput<KeyCode>>()
                    .just_pressed(key),
                "a refused {key:?} was left for the game to read"
            );
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(key);
            keys.clear();
            let menu = app.world().resource::<Menu>();
            assert_eq!(
                menu.capture,
                Some(Action::Raise),
                "{key:?} ended the waiting"
            );
            assert!(menu.refused > 0.0, "{key:?} was refused with no slash");
            assert!(
                app.world().resource::<Bindings>().defaults(),
                "{key:?} was bound"
            );
        }
        tap(&mut app, KeyCode::KeyV);
        assert_eq!(
            app.world().resource::<Bindings>().key(Action::Raise),
            KeyCode::KeyV
        );
    }

    /// **The keys page, standing**: the keyboard face turns the page, and
    /// on it every action has its glyph and its keycap, and every keycap
    /// prints its key's name; a waiting cap prints nothing and its
    /// action's glyph is lit; and the page is laid out only while it is
    /// the page, so the panel is the height of the half it shows.
    #[test]
    fn the_keys_page_prints_every_keycap() {
        let mut app = built(true, false, false, false);
        let shown = |app: &mut App, page: Page| {
            app.world_mut()
                .query::<(&Page, &Node)>()
                .iter(app.world())
                .find(|(at, _)| **at == page)
                .is_some_and(|(_, node)| node.display != Display::None)
        };
        assert!(shown(&mut app, Page::Reading) && !shown(&mut app, Page::Keys));
        app.world_mut()
            .spawn((Interaction::Pressed, Paint::Face(Control::Keys)));
        app.update();
        assert!(
            shown(&mut app, Page::Keys) && !shown(&mut app, Page::Reading),
            "the keyboard face did not turn the page"
        );
        let caps = |app: &mut App| -> Vec<(Action, String)> {
            app.world_mut()
                .query::<(&CapText, &Text)>()
                .iter(app.world())
                .map(|(cap, text)| (cap.0, text.0.clone()))
                .collect()
        };
        let printed = caps(&mut app);
        assert_eq!(
            printed.len(),
            Action::ALL.len(),
            "a row is missing its keycap"
        );
        for (action, name) in printed {
            assert_eq!(
                Some(name.as_str()),
                crate::keys::name(action.default_key()),
                "{action:?}'s keycap"
            );
        }
        for action in Action::ALL {
            let runs = app
                .world_mut()
                .query::<&Paint>()
                .iter(app.world())
                .filter(|paint| matches!(paint, Paint::Mark(of) if *of == action))
                .count();
            assert!(runs > 0, "{action:?} has no glyph");
        }
        app.world_mut()
            .spawn((Interaction::Pressed, Paint::Keycap(Action::TurnCw)));
        app.update();
        let waiting = caps(&mut app)
            .into_iter()
            .find(|(action, _)| *action == Action::TurnCw)
            .map(|(_, name)| name);
        assert_eq!(
            waiting.as_deref(),
            Some(""),
            "a waiting cap still printed its key"
        );
    }

    /// The whole menu standing in a bare app: `spawn` builds it, `paint`
    /// reads the sim back onto it, and nothing else in the world spawns
    /// anything at all. Every entity here is therefore a piece of the
    /// menu, which is the point — a piece spawned outside the root is
    /// one of the ways the menu could leave something behind, and a
    /// query that only walked the root's children would never see it.
    // Four yes/no facts about one frame of the world, which is exactly
    // what the cross product below is made of; enums per fact would make
    // those loops harder to read and not one case safer.
    #[allow(clippy::fn_params_excessive_bools)]
    fn built(open: bool, muted: bool, paused: bool, warping: bool) -> App {
        let mut bridge = Bridge::boot_fixture(crate::fixture::SAVE);
        // Toggles, not settings: the sim owns pause and warp, so the
        // only honest way to ask for a state is to work the control.
        let input = InputFrame {
            toggle_pause: bridge.sim.is_paused() != paused,
            toggle_warp: bridge.sim.is_warp() != warping,
            ..InputFrame::default()
        };
        bridge.sim.advance(0.0, &input);
        assert_eq!(bridge.sim.is_paused(), paused, "the sim refused to pause");
        assert_eq!(bridge.sim.is_warp(), warping, "the sim refused to warp");
        let mut app = App::new();
        app.insert_resource(Shell {
            bridge,
            outcome: FrameOutcome::default(),
            muted,
        })
        .insert_resource(Menu::boot(open))
        .insert_resource(CameraRig::boot(None))
        .insert_resource(Bindings::default())
        .add_systems(PostStartup, (spawn, mark))
        .add_systems(Update, (click, paint, show_mark).chain());
        app.update();
        app
    }

    /// Bevy's rule for whether a node reaches the screen, run over the
    /// hierarchy the menu actually built: `Visible` draws even under a
    /// hidden parent, `Hidden` stops there, `Inherited` asks its parent,
    /// and an entity with no parent left to ask is up. What the renderer
    /// would make of this menu, decided without one.
    fn drawn(world: &World, entity: Entity) -> bool {
        match world.get::<Visibility>(entity) {
            None | Some(Visibility::Hidden) => false,
            Some(Visibility::Visible) => true,
            Some(Visibility::Inherited) => world
                .get::<ChildOf>(entity)
                .is_none_or(|child_of| drawn(world, child_of.parent())),
        }
    }

    /// Everything the PANEL is putting on the screen this frame. The
    /// mark under the crosshair is not the panel and is left out of
    /// this deliberately — it answers to
    /// `a_world_that_has_stopped_says_so_with_the_menu_shut`, which is
    /// the law that wants it drawn.
    fn panel_on_screen(app: &App) -> Vec<Entity> {
        let world = app.world();
        world
            .iter_entities()
            .map(|entity| entity.id())
            .filter(|entity| world.get::<Telltale>(*entity).is_none() && drawn(world, *entity))
            .collect()
    }

    /// The mark as a player would meet it: every piece of it that
    /// reaches the screen, described by the rectangle it puts there.
    /// Colour is not collected, because the reading may not rest on it.
    fn mark_shape(app: &App) -> Vec<String> {
        let world = app.world();
        let mut cells: Vec<String> = world
            .iter_entities()
            .filter(|entity| entity.contains::<Telltale>() && drawn(world, entity.id()))
            .filter_map(|entity| {
                let node = entity.get::<Node>()?;
                Some(format!(
                    "{:?} {:?} {:?} {:?}",
                    node.left, node.top, node.width, node.height
                ))
            })
            .collect();
        cells.sort();
        cells
    }

    /// How many runs of the refusal slash are on the screen.
    fn slash_on_screen(app: &App) -> usize {
        let world = app.world();
        world
            .iter_entities()
            .filter(|entity| entity.contains::<Slash>() && drawn(world, entity.id()))
            .count()
    }

    /// **A closed menu draws nothing.** The controls are things done
    /// *to* the game, and the overlay is the only place any of them may
    /// be worked: with it shut the player is looking at the cabin and at
    /// no button at all.
    ///
    /// The line this once drew was one step further out — nothing
    /// whatever on the screen — and pause is why it moved. A control may
    /// not leak, because a control off its panel is a HUD; a state the
    /// player has no other way to perceive must, because a state nobody
    /// can perceive is the defect that cost an evening. Mute sits on the
    /// near side of that line and keeps nothing outside (it is audible),
    /// and the mark that sits on the far side is asked about by name in
    /// `a_world_that_has_stopped_says_so_with_the_menu_shut`. What is
    /// asserted here is everything else: the faces, the lamps, the
    /// slash, the tally, in every state the sim can hand them.
    ///
    /// Mute broke this first. Its refusal slash asked for
    /// `Visibility::Visible`, which in Bevy means *draw me even though
    /// an ancestor is hidden*, so muting and then closing the menu left
    /// a red bar hanging over the played frame. The law is not about
    /// mute, though, which is why this asks it of every entity the menu
    /// owns in every state the sim can hand it — a slash that can escape
    /// is a lamp that can escape.
    #[test]
    fn a_closed_menu_draws_nothing() {
        for muted in [false, true] {
            for paused in [false, true] {
                for warping in [false, true] {
                    let app = built(false, muted, paused, warping);
                    let escaped = panel_on_screen(&app);
                    assert!(
                        escaped.is_empty(),
                        "a closed menu put {} node(s) over the game \
                         (muted {muted}, paused {paused}, warping {warping})",
                        escaped.len()
                    );
                }
            }
        }
    }

    /// **A world that has stopped says so, with the menu shut.**
    ///
    /// This is the defect, stated. Pause changes what every click in the
    /// game does and changed nothing about the picture: the cabin keeps
    /// breathing on the wall clock, the crosshair keeps finding things,
    /// and the presses land nowhere. The only way to learn the world had
    /// stopped was to open the panel that stopped it. Fast-forward had
    /// the same hole with the panel's own excuse and none of the
    /// harmlessness — a world at sixteen times speed does things nobody
    /// watching can account for.
    ///
    /// The question asked is the player's: with the panel down, what
    /// reaches the screen? Every node is resolved by Bevy's own
    /// inheritance rule over the hierarchy the game really builds, and
    /// the marks are compared as the rectangles they put on the glass —
    /// never as the branch that chose them, and never by colour, which
    /// is the one thing this reading is not allowed to rest on.
    #[test]
    fn a_world_that_has_stopped_says_so_with_the_menu_shut() {
        let running = mark_shape(&built(false, false, false, false));
        assert!(
            running.is_empty(),
            "a world running as it should marked itself anyway"
        );

        let paused = mark_shape(&built(false, false, true, false));
        assert!(
            !paused.is_empty(),
            "the world stopped and the screen said nothing"
        );

        let warping = mark_shape(&built(false, false, false, true));
        assert!(
            !warping.is_empty(),
            "the world ran at speed and the screen said nothing"
        );

        assert_ne!(
            paused, warping,
            "stopped and racing drew the same rectangles; \
             only the colour told them apart"
        );

        // Both at once: nothing is advancing, which is the louder fact
        // and the one the sim itself acts on.
        assert_eq!(
            mark_shape(&built(false, false, true, true)),
            paused,
            "a paused world that is also warped must read as stopped"
        );

        // The mark is the world's, not the panel's: what the speaker is
        // doing has no bearing on it.
        assert_eq!(
            mark_shape(&built(false, true, true, false)),
            paused,
            "muting changed what a stopped world looks like"
        );
    }

    /// The other half of the same law, and the reason the first half is
    /// not satisfied by a menu that never draws: standing, the menu is
    /// on the screen, and the slash is on the speaker exactly when the
    /// sound has stopped.
    #[test]
    fn an_open_menu_wears_its_slash_only_while_muted() {
        let loud = built(true, false, false, false);
        assert!(
            !panel_on_screen(&loud).is_empty(),
            "an open menu must be on the screen"
        );
        assert_eq!(
            slash_on_screen(&loud),
            0,
            "the ship can be heard; nothing refuses"
        );
        let muted = built(true, true, false, false);
        assert!(
            slash_on_screen(&muted) > 0,
            "muted, the speaker must wear its slash"
        );
    }
}
