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
//! **Zero text, like everything else, but for one label** (DESIGN.md's
//! law: nothing renders a string except the version corner and the
//! new-run label). The icons are *stamped* — a bit per cell, one `u16`
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
//! `Esc` semantics, preserved rather than replaced: focused at a
//! station, `Esc` steps out first (the room before the meta); roaming,
//! `Esc` opens this and frees the cursor — which is exactly the parking
//! `Esc` always did, only now there is something to click. `Esc` again,
//! or a click on the bare scrim, puts it away and takes the cursor back.

// The glyph rows ARE the pictures: a separator inside `0b111011100`
// breaks the correspondence between the literal and the shape it draws.
// Same bargain `palette` strikes with its hex.
#![allow(clippy::unreadable_literal)]

use bevy::prelude::*;

use crate::console::DELIVERY_LAMPS;
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

/// The menu's one string. Plain and short, and kept in this one place:
/// DESIGN.md asks that text the game cannot avoid be translatable, and a
/// translation of this is a change to this line.
const NEW_RUN: &str = "New run";

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

/// Which meta-control a face, glyph cell, or lamp belongs to.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum Control {
    Pause,
    Warp,
    Mute,
}

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

/// The menu: whether it stands, and what was worked on it this frame.
#[derive(Resource, Default)]
pub struct Menu {
    pub open: bool,
    worked: Worked,
}

impl Menu {
    /// Boot state. `--menu` opens it standing, for screenshot runs.
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
        }
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

    /// Put the menu away and hand the cursor back to the room.
    const fn close(&mut self, rig: &mut CameraRig) {
        self.open = false;
        rig.parked = false;
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
fn spawn(mut commands: Commands, shell: Res<Shell>) {
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

    // A hairline between the controls and the reading: one is something
    // you do, the other is something that happened.
    commands.spawn((
        Node {
            width: percent(100),
            height: px(2),
            ..default()
        },
        BackgroundColor(palette::PLATE_SHADE),
        Pickable::IGNORE,
        ChildOf(panel),
    ));

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
            ChildOf(panel),
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

    new_run(&mut commands, panel);
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

/// `Esc`, and what it means where. Runs first in `Phase::Input`, ahead
/// of the camera: an `Esc` this answers is one `rig::steer` must never
/// also act on, and `steer` returns early while the menu stands.
pub fn keys(keys: Res<ButtonInput<KeyCode>>, mut rig: ResMut<CameraRig>, mut menu: ResMut<Menu>) {
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

/// Clicks on the faces, the new-run bar, and the scrim. A face or the bar
/// throws its edge; the bare scrim dismisses.
fn click(
    mut menu: ResMut<Menu>,
    mut rig: ResMut<CameraRig>,
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
            Paint::NewRun => menu.worked.reseed = true,
            Paint::Ink(_) | Paint::Lamp(_) | Paint::Pip(_) => {}
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

/// The two nodes in the menu that come and go: the root (the whole
/// overlay) and the mute slash. They share a query because two
/// `&mut Visibility` params in one system are a conflict Bevy refuses at
/// startup — and `Has` answers which is which more cheaply than proving
/// two filters disjoint.
type Shown<'w, 's> =
    Query<'w, 's, (&'static mut Visibility, Has<MenuRoot>), Or<(With<MenuRoot>, With<Slash>)>>;

/// The flat equivalent of `glow::set_lamp`: glass at zero, the lamp's
/// own color at one. No emissive out here — the menu is not in the room,
/// so it gets no bloom and wants none.
fn lamp_color(color: Color, level: f32) -> Color {
    palette::mix(palette::GLASS, color, level.clamp(0.0, 1.0))
}

/// Read the sim back onto the menu: icon inks, state lamps, the mute
/// slash, and the tally. Everything it shows, it shows because the sim
/// says so this frame — nothing here caches a toggle of its own.
fn paint(
    shell: Res<Shell>,
    menu: Res<Menu>,
    mut shown: Shown,
    mut parts: Query<(&Paint, Option<&Interaction>, &mut BackgroundColor)>,
) {
    let muted = shell.muted;
    for (mut visibility, is_root) in &mut shown {
        // The root stands or does not; the slash rides mute. They hide
        // by the same word but they show by different ones, and the
        // difference is the whole of it: `Visible` means *drawn even
        // though an ancestor is hidden*, which is the right answer for
        // an overlay that answers to nothing and the wrong one for a
        // mark inside it. Said of the slash it put a red bar across the
        // played frame every time a player muted and shut the menu.
        // Under the root, showing is `Inherited`: the slash says mute is
        // on, the root says whether anyone is being told.
        *visibility = if is_root {
            if menu.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            }
        } else if muted {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !menu.open {
        return;
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
    };
    // Which face the cursor rests on, spent below on that control's
    // lamp — so the tell reaches the whole button, not just its border.
    let hovered = parts
        .iter()
        .find_map(|(paint, interaction, _)| match (paint, interaction) {
            (Paint::Face(control), Some(Interaction::Hovered | Interaction::Pressed)) => {
                Some(*control)
            }
            _ => None,
        });

    for (paint, interaction, mut background) in &mut parts {
        background.0 = match paint {
            Paint::Face(_) => {
                if matches!(
                    interaction,
                    Some(Interaction::Hovered | Interaction::Pressed)
                ) {
                    palette::PLATE_LIT
                } else {
                    palette::PLATE
                }
            }
            // A live function wears its lamp's own color; a sleeping one
            // stays etched metal.
            Paint::Ink(control) => match control {
                Control::Pause | Control::Warp => {
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
                    Control::Pause | Control::Warp => {
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
                if matches!(
                    interaction,
                    Some(Interaction::Hovered | Interaction::Pressed)
                ) {
                    palette::PLATE
                } else {
                    palette::SOCKET
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
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (keys, click).chain());
        app
    }

    /// Press `Esc`, run one frame, and let the key back up — a key held
    /// down never presses twice, which is the whole reason this is a
    /// helper and not two lines inlined.
    fn escape(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::Escape);
        keys.clear();
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

    /// The menu renders one string, and it is the new-run label. The law
    /// is DESIGN.md's ("absolutely no text"). The version corner and this
    /// label are its two exemptions, and this file is exactly the kind of
    /// place that would quietly grow a third — a menu is what menus are
    /// usually made of words for.
    #[test]
    fn the_menu_speaks_in_shapes_but_for_one_label() {
        let source = include_str!("menu.rs");
        // Spelled in pieces so the test does not trip over itself.
        let text_node = ["Text", "::new"].concat();
        assert_eq!(
            source.matches(&text_node).count(),
            1,
            "the menu grew a rendered string; icons only, the new-run label excepted (DESIGN.md)"
        );
        assert!(
            source.contains(&[text_node.as_str(), "(NEW_RUN)"].concat()),
            "the menu's one string is not the new-run label"
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
        .add_systems(PostStartup, (spawn, mark))
        .add_systems(Update, (paint, show_mark));
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
