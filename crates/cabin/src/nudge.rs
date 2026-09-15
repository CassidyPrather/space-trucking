//! **The bench: putting a purchased body where it belongs by standing in
//! front of it.**
//!
//! A bought mesh arrives at whatever size, turn and centre its exporter
//! chose, and the four numbers that put it in its berth
//! (`crate::art`, "The placement frame") have to come from somewhere.
//! Until now that somewhere was a text editor, a guess, a relaunch, and
//! a look — which is a loop about a minute long for a change worth a
//! thousandth of a berth. This is the same loop with the editor and the
//! relaunch taken out: take the body under the crosshair, nudge it with
//! six keys until it sits right, and write the numbers back into
//! `art/manifest.toml` where they ride version control like every other
//! promise this repository makes.
//!
//! # Two layers, and the seam between them is the point
//!
//! **The gesture and the transform are the whole of this file's first
//! half**, and it is compiled into every build — including the whitebox
//! one, where nothing can call it. That is deliberate. Cargo lives on a
//! grid today; the day the grid comes out (`docs/DESIGN_REVIEW.md`'s
//! deferred list) free placement needs exactly these tools — take a
//! thing, move it, turn it, size it, put it back, keep it — and none of
//! what they need is about TOML. So the state machine knows nothing
//! about files, the writer ([`crate::art::rewritten`]) knows nothing
//! about gestures, and the two meet at [`Adjust::lines`], which is three
//! key names and three triples.
//!
//! **The bench half is behind `--features art` and an explicit
//! `--nudge`**, and both halves of that gate earn their keep. The
//! feature is what makes a purchased mesh exist to be nudged at all; the
//! flag is what makes an ordinary session *incapable* of writing to the
//! manifest, because the systems below are never added to the schedule
//! without it. A key chord would have been fewer characters to type and
//! would have left the file-writing code live in every session anybody
//! ever plays, one stuck modifier away from a silent edit to a tracked
//! file. A flag cannot be pressed by accident.
//!
//! # What the hands do
//!
//! | | |
//! | --- | --- |
//! | `Tab` | take the dressed body under the crosshair, or let go |
//! | `T` `R` `G` | what the six direction keys move: offset, rotation, scale |
//! | `←` `→` | the berth's own x, minus and plus |
//! | `↑` `↓` | its y, plus and minus |
//! | `[` `]` | its z, minus and plus — into the wall and out of it |
//! | `Shift` | the fine step |
//! | `Backspace` | put the numbers back to what the file says |
//! | `Enter` | write them into the manifest |
//!
//! **The arrows move the body in the berth's own axes, not in yours.**
//! Standing behind a body on the aft wall, a press of `→` moves it to
//! your left, and that is correct: what the key moves is the *number*,
//! in the frame the number is written in, so what you press is what the
//! diff says. The alternative — turning the camera's right into the
//! berth's — would hide which of the three numbers is moving, and this
//! bench exists to author numbers. Which way is plus is not left to be
//! remembered: the overlay draws a tip on the plus end of every axis.
//!
//! One press is one step. There is no key repeat, because a held arrow
//! at sixty steps a second crosses a whole berth in a third of a second
//! and no hand can stop it where it wanted to.
//!
//! # What it draws, and what it will not
//!
//! The zero-text law covers everything drawn, so the overlay speaks in
//! shapes: rods along the axes for the offset, rings round them for the
//! turn, calipers across them for the size, a tip on every plus end, and
//! above the body a ring that is **whole when what you are looking at is
//! what the file says and broken when it is not**. [`Mark`] carries no
//! colour at all, which is the strongest form of the no-hue-alone rule
//! available: a description with no hue in it cannot be telling anything
//! by hue.
//!
//! Complaints, confirmations and the file names go to **stderr**, which
//! the law has never covered and which is where the rest of this
//! pipeline already talks.
//!
//! # What a save does and does not do
//!
//! It writes `scale`, `offset` and `rotation` into the asset's table in
//! the manifest — a surgical line edit that leaves every other byte of
//! the owner's file alone — and then carries the same three lines into
//! `$ART_CACHE/index.toml`, which `resolve` owns and rewrites, so the
//! body is still where you put it after a restart rather than back where
//! the last resolve left it.
//!
//! It does **not** write `fill`. `fill` is the promise the mesh is
//! measured against, and a bench that derived it from the mesh would
//! make it unbreakable, which is precisely the thing it exists not to be
//! (`docs/DESIGN_REVIEW.md`). Nudging `scale` can therefore leave a `fill`
//! that is no longer true; the save says so on stderr, and the next
//! `cargo xtask art resolve` refuses with the line to paste.
//!
//! Nothing here reaches the sim. No save format moves, no input frame is
//! synthesized, no schedule the sim reads is touched: the bench runs in
//! `Phase::View`, after the world has already decided what happened.

// The gesture and transform half is compiled everywhere and called only
// under the feature — the same trade `crate::art`'s declaration half
// makes, and for the same reason: the tools outlive the manifest.
#![cfg_attr(not(feature = "art"), allow(dead_code))]

use bevy::prelude::*;
use space_trucking::sim::Kind;

use crate::art::Dressing;
use crate::pieces::Body;

// ------------------------------------------------- the numbers in hand --

/// **Which of the three numbers the six direction keys move.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Handle {
    /// Where the body's middle sits in its berth.
    #[default]
    Move,
    /// Degrees about the berth's own axes.
    Turn,
    /// The converted file's units, in berth half-units.
    Size,
}

impl Handle {
    /// **How far one press moves this number**: the coarse step, and the
    /// fine one a held `Shift` asks for.
    ///
    /// A berth half-unit is about 275 mm on a one-cell kind, so a coarse
    /// move is fourteen millimetres — visible from across the cabin —
    /// and a fine one is a millimetre and a half, which is under the
    /// slack the resolver measures a `fill` to. Six coarse turns make a
    /// quarter turn, which is the turn a mesh most often wants, and a
    /// fine one is the degree a mesh is most often out by.
    ///
    /// **A coarse step is a whole number of fine ones**, on all three,
    /// so mixing them can never leave a number off the grid the file is
    /// written on.
    const fn step(self) -> (f32, f32) {
        match self {
            Self::Move | Self::Size => (0.05, 0.005),
            Self::Turn => (15.0, 1.0),
        }
    }
}

/// **The three numbers a body is placed with**, in the placement frame
/// `crate::art` documents. `fill` is not among them and never will be:
/// it is the promise, not the placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adjust {
    pub offset: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

/// **The smallest a body may be nudged to.** A scale of zero is a mesh
/// with no volume, no normals worth the name, and no way to see what you
/// are doing to it; a body nudged into nothing would have to be got back
/// out of a file by hand.
const SIZE_FLOOR: f32 = 0.01;

/// **The grid every nudged number lands on**: a thousandth.
///
/// Not decoration. A step is added to an `f32` and the sum is written
/// into a file a person reads and a diff shows; without this the fourth
/// press of `↑` writes `0.20000002` into the owner's manifest and every
/// later diff carries it. A thousandth of a berth half-unit is a third
/// of a millimetre, which is finer than the finest step and far finer
/// than the resolver's own slack.
const GRID: f64 = 1000.0;

impl Adjust {
    /// What a declaration says today.
    #[must_use]
    pub const fn of(dressing: &Dressing) -> Self {
        Self {
            offset: dressing.offset,
            rotation: dressing.rotation,
            scale: dressing.scale,
        }
    }

    /// The same declaration with these numbers in it: what the body is
    /// drawn with while it is in hand, and what the file gets.
    #[must_use]
    pub fn onto(self, dressing: &Dressing) -> Dressing {
        Dressing {
            offset: self.offset,
            rotation: self.rotation,
            scale: self.scale,
            ..dressing.clone()
        }
    }

    /// Where these numbers stand the mesh, through the loader's own
    /// arithmetic and never a second copy of it.
    #[must_use]
    pub fn pose(self, dressing: &Dressing, kind: Kind) -> Transform {
        self.onto(dressing).pose(kind)
    }

    /// **The seam.** Three key names and three triples — the whole of
    /// what the manifest-writing layer is ever told about a gesture, and
    /// the whole of what this layer knows about a file.
    ///
    /// In the order the manifest itself writes them, so a table that
    /// gains all three at once gains them the way one written by hand
    /// would read.
    #[must_use]
    pub const fn lines(&self) -> [(&'static str, Vec3); 3] {
        [
            ("scale", self.scale),
            ("offset", self.offset),
            ("rotation", self.rotation),
        ]
    }

    /// One step along one axis of one number.
    fn step(&mut self, handle: Handle, axis: usize, way: f32, fine: bool) {
        let (coarse, small) = handle.step();
        let number = match handle {
            Handle::Move => &mut self.offset,
            Handle::Turn => &mut self.rotation,
            Handle::Size => &mut self.scale,
        };
        let moved = way.mul_add(if fine { small } else { coarse }, number[axis]);
        number[axis] = if matches!(handle, Handle::Size) {
            snap(moved).max(SIZE_FLOOR)
        } else {
            snap(moved)
        };
    }
}

/// The nearest thousandth, computed wide so that the answer is the
/// nearest `f32` to a round decimal rather than the nearest `f32` to a
/// sum of `f32` steps.
fn snap(value: f32) -> f32 {
    #[allow(clippy::cast_possible_truncation)] // back to where it came from
    {
        ((f64::from(value) * GRID).round() / GRID) as f32
    }
}

// ------------------------------------------------------ the vocabulary --

/// **One thing the hands can ask for.**
///
/// The gestures are data, so the state machine below can be driven with
/// no keyboard, no window and no game — and so the keys can be re-cut
/// (or a gamepad grown) without touching a line of what they mean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ask {
    /// Take the dressed body under the crosshair, or let go of the one
    /// in hand.
    Take,
    /// Which number the six direction keys move from here on.
    Hold(Handle),
    /// One step: `axis` is 0, 1 or 2 of the berth's own frame, `way` is
    /// ±1, and `fine` is the small step.
    Step { axis: usize, way: f32, fine: bool },
    /// Put the numbers back to what the file says.
    Undo,
    /// Write them into the file.
    Save,
}

/// **The six directions**, as key, axis, and which way along it.
///
/// Arrows for the two axes a wall shows you and brackets for the one it
/// does not: `[` pushes a body back into the chart it hangs on and `]`
/// pulls it out into the room, which is the way the keys lean.
const DIRECTIONS: [(KeyCode, usize, f32); 6] = [
    (KeyCode::ArrowLeft, 0, -1.0),
    (KeyCode::ArrowRight, 0, 1.0),
    (KeyCode::ArrowDown, 1, -1.0),
    (KeyCode::ArrowUp, 1, 1.0),
    (KeyCode::BracketLeft, 2, -1.0),
    (KeyCode::BracketRight, 2, 1.0),
];

/// Which key names which number. `T`, `R` and `G` are neighbours under
/// the same hand that walks, and none of them is a key the cabin
/// already answers.
const HANDLES: [(KeyCode, Handle); 3] = [
    (KeyCode::KeyT, Handle::Move),
    (KeyCode::KeyR, Handle::Turn),
    (KeyCode::KeyG, Handle::Size),
];

/// Take and let go.
const TAKE: KeyCode = KeyCode::Tab;
/// Put the numbers back.
const UNDO: KeyCode = KeyCode::Backspace;
/// Write them down.
const SAVE: KeyCode = KeyCode::Enter;
/// Held, the step is a tenth.
const FINE: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];

/// **Read a frame of keys into asks.** Edges only: one press is one
/// step, and a key held down asks once.
#[must_use]
pub fn asked(keys: &ButtonInput<KeyCode>) -> Vec<Ask> {
    let fine = FINE.iter().any(|key| keys.pressed(*key));
    let mut out = Vec::new();
    if keys.just_pressed(TAKE) {
        out.push(Ask::Take);
    }
    for (key, handle) in HANDLES {
        if keys.just_pressed(key) {
            out.push(Ask::Hold(handle));
        }
    }
    for (key, axis, way) in DIRECTIONS {
        if keys.just_pressed(key) {
            out.push(Ask::Step { axis, way, fine });
        }
    }
    if keys.just_pressed(UNDO) {
        out.push(Ask::Undo);
    }
    if keys.just_pressed(SAVE) {
        out.push(Ask::Save);
    }
    out
}

// -------------------------------------------------- the state in hand --

/// **The body in hand**: whose numbers are being moved, what the file
/// said when it was taken, and what the hands have made of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    /// The asset's stable id — which `[asset.<id>]` table a save writes.
    pub id: String,
    /// The body it dresses. Every copy of that kind in the ship moves
    /// together, because one declaration dresses them all.
    pub kind: Kind,
    /// What the file says.
    pub was: Adjust,
    /// What the hands have made of it.
    pub now: Adjust,
}

impl Held {
    /// **Whether what is on screen is also what is in the file.** Not a
    /// flag: a flag can disagree with the numbers, and this cannot.
    #[must_use]
    pub fn kept(&self) -> bool {
        self.now == self.was
    }
}

/// **The bench's whole state**, and it is all here: nothing the sim can
/// see, nothing a save file carries, nothing a replay would have to
/// reproduce.
#[derive(Resource, Default, Debug)]
pub struct Nudge {
    pub held: Option<Held>,
    pub handle: Handle,
}

impl Nudge {
    /// **Hear one ask.** `under` is the dressed body the crosshair rests
    /// on, if it rests on one.
    ///
    /// Answers whether the file should now be written, because opening a
    /// file is not this layer's business — the caller does that and says
    /// so with [`Self::keep`].
    pub fn hear(&mut self, ask: Ask, under: Option<(Kind, &Dressing)>) -> bool {
        match ask {
            // Taking what is already in hand is letting go of it, and
            // taking with nothing under the crosshair is letting go too:
            // one key, and the answer to "how do I put this down" is the
            // same key wherever you happen to be looking.
            Ask::Take => {
                let taking = under
                    .filter(|(kind, _)| self.held.as_ref().is_none_or(|held| held.kind != *kind));
                self.held = taking.map(|(kind, dressing)| Held {
                    id: dressing.id.clone(),
                    kind,
                    was: Adjust::of(dressing),
                    now: Adjust::of(dressing),
                });
            }
            Ask::Hold(handle) => self.handle = handle,
            Ask::Step { axis, way, fine } => {
                let handle = self.handle;
                if let Some(held) = &mut self.held {
                    held.now.step(handle, axis, way, fine);
                }
            }
            Ask::Undo => {
                if let Some(held) = &mut self.held {
                    held.now = held.was;
                }
            }
            // A save that would write what is already written is not
            // asked for; the mark is already whole.
            Ask::Save => return self.held.as_ref().is_some_and(|held| !held.kept()),
        }
        false
    }

    /// **The file now says what the hands say.** Told to the state
    /// machine by whoever wrote the file, so that the mark closes and an
    /// undo goes back to the numbers that are actually on disk.
    pub const fn keep(&mut self) {
        if let Some(held) = &mut self.held {
            held.was = held.now;
        }
    }
}

// ---------------------------------------------------------- the overlay --

/// **One mark of the overlay**: a shape, and where it stands in the
/// berth's own frame.
///
/// There is no colour in it, and that is the point rather than an
/// omission — every reading this overlay carries is carried by form, so
/// there is no hue for one to hide in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub body: Body,
    pub at: Transform,
}

/// How thick a rod is, as a fraction of the berth box's smallest half.
const ROD: f32 = 0.04;
/// How far past the berth box the axes reach, so a body that fills its
/// berth does not swallow its own overlay.
const PROUD: f32 = 1.12;

/// **What the overlay draws.** Nothing in hand, nothing drawn.
#[must_use]
pub fn overlay(nudge: &Nudge) -> Vec<Mark> {
    let Some(held) = &nudge.held else {
        return Vec::new();
    };
    let (mid, half) = Dressing::berth_box(held.kind);
    let rod = half.min_element() * ROD;
    // A body `len` long on one axis and a rod thick on the other two.
    let along = |axis: usize, len: f32| {
        let mut size = Vec3::splat(rod);
        size[axis] = len;
        size
    };
    // Its opposite: a plate a rod thick, facing along the axis.
    let across = |axis: usize, span: f32| {
        let mut size = Vec3::splat(span);
        size[axis] = rod;
        size
    };
    let at = |axis: usize, out: f32| {
        let mut place = mid;
        place[axis] += out;
        Transform::from_translation(place)
    };
    let mut marks = Vec::new();
    for axis in 0..3 {
        let reach = half[axis] * PROUD;
        // Which way is plus, said the same way in every handle: a tip on
        // the plus end and nothing on the minus one.
        marks.push(Mark {
            body: Body::Box(Vec3::splat(rod * 2.5)),
            at: at(axis, reach),
        });
        match nudge.handle {
            // A rod through the box, along the axis the number moves.
            Handle::Move => marks.push(Mark {
                body: Body::Box(along(axis, reach * 2.0)),
                at: at(axis, 0.0),
            }),
            // A ring round the axis it turns about, lying in the plane
            // it sweeps — so its radius is the plane's, not the axis's,
            // which is the difference between a circle and an ellipse
            // on the kinds whose berth is twice as tall as it is wide.
            Handle::Turn => marks.push(Mark {
                body: ring(
                    f32::midpoint(half[(axis + 1) % 3], half[(axis + 2) % 3]) * PROUD,
                    rod,
                ),
                at: at(axis, 0.0).with_rotation(ring_turn(axis)),
            }),
            // Calipers: a plate across each end of the axis, closing on
            // the body the way a gauge does.
            Handle::Size => {
                for way in [-1.0, 1.0] {
                    marks.push(Mark {
                        body: Body::Box(across(axis, rod * 6.0)),
                        at: at(axis, reach * way),
                    });
                }
            }
        }
    }
    marks.extend(state_mark(mid, half, rod, held.kept()));
    marks
}

/// **A ring of radius `r`, a rod thick.** A hoop's two numbers are its
/// inner and outer radius, so a tube of a given thickness is a pair
/// straddling the circle it is drawn on — and `Hoop { inner: rod, .. }`,
/// which is what it looks like it should be, is a doughnut.
fn ring(r: f32, rod: f32) -> Body {
    Body::Hoop {
        inner: r - rod,
        outer: r + rod,
    }
}

/// The turn that lays a hoop — which lies in its own `x/z` — in the
/// plane an axis sweeps.
fn ring_turn(axis: usize) -> Quat {
    match axis {
        0 => Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        1 => Quat::IDENTITY,
        _ => Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
    }
}

/// How many dashes a broken ring is cut into.
const DASHES: usize = 8;

/// **The one reading that is not about an axis**: a ring standing over
/// the body, whole when the file agrees with the screen and broken into
/// dashes when it does not.
///
/// Broken-means-provisional is the cabin's own vocabulary, not a new
/// one: it is what a room's mark on a good already says
/// (`crate::outline`), and a ring somebody has to learn is a ring
/// nobody reads.
fn state_mark(mid: Vec3, half: Vec3, rod: f32, kept: bool) -> Vec<Mark> {
    let round = half.x.min(half.y) * 0.3;
    let over = mid + Vec3::Y * rod.mul_add(3.0, half.y.mul_add(PROUD, round));
    if kept {
        return vec![Mark {
            body: ring(round, rod),
            // Face-on to the room, which is the way a body on a wall is
            // looked at.
            at: Transform::from_translation(over)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        }];
    }
    (0..DASHES)
        .map(|nth| {
            #[allow(clippy::cast_precision_loss)] // eight of them
            let turn = std::f32::consts::TAU * (nth as f32 + 0.5) / DASHES as f32;
            Mark {
                body: Body::Box(Vec3::new(rod, round * 0.5, rod)),
                at: Transform::from_translation(
                    over + Vec3::new(turn.cos() * round, turn.sin() * round, 0.0),
                )
                .with_rotation(Quat::from_rotation_z(turn)),
            }
        })
        .collect()
}

// -------------------------------------------------------- the bench --

#[cfg(feature = "art")]
pub use bench::plugin;

#[cfg(feature = "art")]
mod bench {
    use std::path::PathBuf;

    use bevy::prelude::*;
    use space_trucking::sim::layout;

    use super::{Adjust, Mark, Nudge, asked, overlay};
    use crate::art::{Dressings, Worn};
    use crate::palette;
    use crate::surface::VirtualPointer;
    use crate::{Phase, Shell};

    /// **Which files the bench writes**, as a resource for exactly the
    /// reason `crate::art`'s own `Cache` is one: a guard cannot set an
    /// environment variable — this workspace forbids `unsafe` and
    /// `std::env::set_var` is unsafe — so a bench that could only ever
    /// be pointed at the real manifest would be a bench nothing could
    /// test.
    #[derive(Resource, Debug, Clone)]
    pub struct Bench {
        /// The file in git: the one that matters, and the one a refusal
        /// is about.
        pub manifest: PathBuf,
        /// The file `resolve` owns and rewrites. Written best-effort, so
        /// that a restart shows what was saved rather than what the last
        /// resolve left.
        pub index: PathBuf,
    }

    impl Default for Bench {
        fn default() -> Self {
            Self {
                manifest: crate::art::manifest_path(),
                index: crate::art::cache_root().join("index.toml"),
            }
        }
    }

    /// **The bench, armed.** Called from `main` behind `--nudge` and
    /// from nowhere else, which is the whole of what stops an ordinary
    /// session from being able to write to a tracked file: without the
    /// flag there is no system in the schedule that can open one.
    pub fn plugin(app: &mut App) {
        let bench = Bench::default();
        eprintln!(
            "nudge: the bench is armed. Tab takes the body under the crosshair; T R G choose \
             offset, rotation, scale; arrows and brackets step its three axes; Shift is the \
             fine step; Backspace puts it back; Enter writes {} (and {}).",
            bench.manifest.display(),
            bench.index.display()
        );
        app.insert_resource(bench)
            .init_resource::<Nudge>()
            .init_resource::<Drawn>()
            // **`Phase::View`, and that is a claim rather than a
            // convenience.** Everything here runs after the sim has
            // already advanced on this frame's input, so there is no
            // ordering in which a nudge key could reach `advance` at
            // all. The bench is presentation writing to a dev file.
            .add_systems(Update, (hands, wear, draw).chain().in_set(Phase::View));
    }

    /// Read the keys, move the numbers, and write the file when asked.
    pub(super) fn hands(
        keys: Res<ButtonInput<KeyCode>>,
        menu: Res<crate::menu::Menu>,
        pointer: Res<VirtualPointer>,
        shell: Res<Shell>,
        bench: Res<Bench>,
        mut dressings: ResMut<Dressings>,
        mut nudge: ResMut<Nudge>,
    ) {
        // While the menu stands it owns the keyboard, exactly as it does
        // for the camera (`rig::steer`).
        if menu.open {
            return;
        }
        let asks = asked(&keys);
        if asks.is_empty() {
            return;
        }
        let sim = &shell.bridge.sim;
        // The crosshair answers by the whitebox box even where a bought
        // mesh is drawn — a bounded, recorded gap (docs/GAUNTLET.md's
        // blind-spot list), and not one to close in passing here.
        let under =
            layout::piece_at(sim.rooms(), sim.pieces(), pointer.sim).map(|piece| piece.kind);
        for ask in asks {
            let subject = under.and_then(|kind| dressings.of(kind).map(|worn| (kind, worn)));
            if nudge.hear(ask, subject) {
                write(&bench, &mut nudge, &mut dressings);
            }
        }
    }

    /// **Write the three lines**, and say what happened on stderr.
    fn write(bench: &Bench, nudge: &mut Nudge, dressings: &mut Dressings) {
        let Some(held) = nudge.held.clone() else {
            return;
        };
        let lines = held.now.lines();
        if let Err(why) = crate::art::save_into(&bench.manifest, &held.id, &lines) {
            eprintln!("nudge: nothing was written — {why}");
            return;
        }
        eprintln!(
            "nudge: {} now says {:?} is scale {:?}, offset {:?}, rotation {:?}",
            bench.manifest.display(),
            held.id,
            held.now.scale,
            held.now.offset,
            held.now.rotation
        );
        // The derived file, best effort: `resolve` rewrites it from the
        // manifest anyway, so a failure here costs a restart's fidelity
        // and nothing else.
        if let Err(why) = crate::art::save_into(&bench.index, &held.id, &lines) {
            eprintln!(
                "nudge: the manifest is written; the index is not ({why}). \
                 `cargo xtask art resolve` writes it."
            );
        }
        if held.now.scale != held.was.scale {
            eprintln!(
                "nudge: `scale` moved, so the `fill` beside it may no longer be true. \
                 `cargo xtask art resolve` measures the mesh and prints the line to paste."
            );
        }
        // What this run believes moves with the file, so letting go does
        // not spring the body back to what the index said at boot.
        if let Some(worn) = dressings.of(held.kind) {
            let moved = held.now.onto(worn);
            dressings.dress(held.kind, moved);
        }
        nudge.keep();
    }

    /// **Stand every dressed body where its numbers say**, the one in
    /// hand included. Which is also how letting go puts a body back: the
    /// held numbers stop applying and the file's own take over again on
    /// the very next frame.
    pub(super) fn wear(
        nudge: Res<Nudge>,
        dressings: Res<Dressings>,
        mut worn: Query<(&Worn, &mut Transform)>,
    ) {
        for (body, mut at) in &mut worn {
            let Some(dressing) = dressings.of(body.0) else {
                continue;
            };
            let live = nudge
                .held
                .as_ref()
                .filter(|held| held.kind == body.0)
                .map_or_else(|| Adjust::of(dressing), |held| held.now);
            let pose = live.pose(dressing, body.0);
            if *at != pose {
                *at = pose;
            }
        }
    }

    /// One mark of the overlay, standing in some rig.
    #[derive(Component)]
    pub(super) struct BenchMark;

    /// What the overlay is currently made of, so a frame that changed
    /// nothing spawns nothing. Without it the bench would cut a dozen
    /// meshes sixty times a second to draw the same picture.
    #[derive(Resource, Default)]
    pub(super) struct Drawn {
        marks: Vec<Mark>,
        rigs: Vec<Entity>,
    }

    /// Stamp the overlay into every rig wearing the held body.
    pub(super) fn draw(
        mut commands: Commands,
        nudge: Res<Nudge>,
        worn: Query<(&Worn, &ChildOf)>,
        standing: Query<Entity, With<BenchMark>>,
        mut drawn: ResMut<Drawn>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut inks: ResMut<Assets<StandardMaterial>>,
    ) {
        let marks = overlay(&nudge);
        // The marks ride the rig rather than the body: the berth box
        // stays where it is and the mesh moves inside it, which is the
        // thing being looked at.
        let mut rigs: Vec<Entity> = nudge.held.as_ref().map_or_else(Vec::new, |held| {
            worn.iter()
                .filter(|(body, _)| body.0 == held.kind)
                .map(|(_, of)| of.parent())
                .collect()
        });
        rigs.sort_unstable();
        if marks == drawn.marks && rigs == drawn.rigs {
            return;
        }
        for mark in &standing {
            commands.entity(mark).despawn();
        }
        // Unlit, because a dev tool that is legible only in a lit room
        // is a dev tool that fails in the furnace with its fire out; and
        // one ink for every mark, because nothing here is told by hue.
        let ink = inks.add(StandardMaterial {
            base_color: palette::GLINT,
            unlit: true,
            ..default()
        });
        for rig in &rigs {
            for mark in &marks {
                commands.spawn((
                    Mesh3d(meshes.add(mark.body.mesh())),
                    MeshMaterial3d(ink.clone()),
                    mark.at,
                    BenchMark,
                    ChildOf(*rig),
                ));
            }
        }
        drawn.marks = marks;
        drawn.rigs = rigs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dressed body to nudge, read through the loader's own parse
    /// rather than built by hand — the numbers a bench moves are the
    /// numbers a file gave, and a fixture assembled in Rust would prove
    /// the arithmetic against itself.
    const DECLARED: &str = "\
# A fixture index, in the dialect the resolver writes.

[asset.crate_small]
glb = \"glb/abc.glb\"
dresses = \"cargo/suspicious_crate\"
scale = [2.0, 2.0, 2.0]
offset = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0]
fill = [1.0, 1.0, 1.0]
measured_mid = [0.0, 0.5, 0.0]
measured_half = [0.5, 0.5, 0.5]
";

    const KIND: Kind = Kind::SuspiciousCrate;

    fn declared() -> Dressing {
        crate::art::Dressings::read(DECLARED)
            .expect("the dialect")
            .of(KIND)
            .expect("the fixture dresses the crate")
            .clone()
    }

    /// A bench with that body in hand.
    fn in_hand() -> (Nudge, Dressing) {
        let dressing = declared();
        let mut nudge = Nudge::default();
        assert!(!nudge.hear(Ask::Take, Some((KIND, &dressing))));
        (nudge, dressing)
    }

    /// **The bench's keys are the bench's alone.**
    ///
    /// A dev mode that quietly stole `W` would be a dev mode that walks
    /// the body across the room while you nudge, and the failure would
    /// read as a physics defect. So the two modules that answer the
    /// keyboard in the game are asked what they already use — read out of their
    /// own source, so a key bound there tomorrow fails this rather than
    /// silently doubling up.
    #[test]
    fn the_benchs_keys_are_the_benchs_alone() {
        let mut taken: Vec<String> = Vec::new();
        for source in [
            include_str!("rig.rs"),
            include_str!("menu.rs"),
            include_str!("gesture.rs"),
        ] {
            for after in source.split("KeyCode::").skip(1) {
                let name: String = after
                    .chars()
                    .take_while(char::is_ascii_alphanumeric)
                    .collect();
                if !name.is_empty() {
                    taken.push(name);
                }
            }
        }
        let mine: Vec<KeyCode> = DIRECTIONS
            .iter()
            .map(|(key, _, _)| *key)
            .chain(HANDLES.iter().map(|(key, _)| *key))
            .chain([TAKE, UNDO, SAVE])
            .chain(FINE)
            .collect();
        for key in &mine {
            let name = format!("{key:?}");
            assert!(
                !taken.contains(&name),
                "the cabin already answers {name}; the bench may not have it too"
            );
        }
        // And no key of the bench's own means two things either.
        let mut seen = mine.clone();
        seen.sort_by_key(|key| format!("{key:?}"));
        seen.dedup();
        assert_eq!(seen.len(), mine.len(), "a bench key means two things");
    }

    /// **Six keys are six directions, three keys are three numbers, and
    /// the modifier is a tenth.**
    ///
    /// The vocabulary, asked of the reader that turns keys into it. Every
    /// direction is one axis and one way along it, all three axes are
    /// covered both ways, and nothing is asked for by a frame with
    /// nothing pressed.
    #[test]
    fn six_keys_are_six_directions_and_three_keys_are_three_numbers() {
        // A fresh keyboard per press: `clear` leaves a key held, and a
        // held key asks for nothing, which is the whole of why there is
        // no key repeat here.
        let tap = |pressed: &[KeyCode]| {
            let mut keys = ButtonInput::<KeyCode>::default();
            for key in pressed {
                keys.press(*key);
            }
            asked(&keys)
        };
        assert!(tap(&[]).is_empty(), "a still keyboard asked for a step");
        let mut reached: Vec<(usize, f32)> = Vec::new();
        for (key, axis, way) in DIRECTIONS {
            assert_eq!(
                tap(&[key]),
                vec![Ask::Step {
                    axis,
                    way,
                    fine: false
                }]
            );
            reached.push((axis, way));
        }
        for axis in 0..3 {
            for way in [-1.0, 1.0] {
                assert!(
                    reached.contains(&(axis, way)),
                    "no key moves axis {axis} by {way}"
                );
            }
        }
        // Both steps are forward, the fine one is the smaller, and a
        // coarse step is a whole number of fine ones — so a number that
        // has been nudged both ways is still on the grid the file is
        // written on.
        for handle in [Handle::Move, Handle::Turn, Handle::Size] {
            let (coarse, fine) = handle.step();
            assert!(0.0 < fine && fine < coarse, "{handle:?}: {coarse}, {fine}");
            assert!(
                ((coarse / fine).round() - coarse / fine).abs() < 1e-4,
                "{handle:?}: a coarse step is {} fine ones",
                coarse / fine
            );
        }
        let (key, axis, way) = DIRECTIONS[0];
        assert_eq!(
            tap(&[FINE[0], key]),
            vec![Ask::Step {
                axis,
                way,
                fine: true
            }]
        );
        for (key, handle) in HANDLES {
            assert_eq!(tap(&[key]), vec![Ask::Hold(handle)]);
        }
        assert_eq!(tap(&[TAKE]), vec![Ask::Take]);
        assert_eq!(tap(&[UNDO]), vec![Ask::Undo]);
        assert_eq!(tap(&[SAVE]), vec![Ask::Save]);
    }

    /// **Nothing is taken until the crosshair rests on something
    /// dressed**, and taking what is already in hand puts it down.
    #[test]
    fn nothing_is_taken_until_the_crosshair_rests_on_something_dressed() {
        let dressing = declared();
        let mut nudge = Nudge::default();
        nudge.hear(Ask::Take, None);
        assert!(nudge.held.is_none(), "the bench took hold of nothing");
        // A step with nothing in hand is a step that moves nothing, and
        // in particular is not a panic.
        nudge.hear(
            Ask::Step {
                axis: 1,
                way: 1.0,
                fine: false,
            },
            None,
        );
        assert!(nudge.held.is_none());

        nudge.hear(Ask::Take, Some((KIND, &dressing)));
        let held = nudge.held.as_ref().expect("the crate is in hand");
        assert_eq!(held.id, "crate_small", "the table a save would write");
        assert_eq!(held.kind, KIND);
        assert_eq!(held.was, Adjust::of(&dressing));
        assert!(
            held.kept(),
            "a body just taken is already what the file says"
        );

        // Taking the same body again is letting go of it.
        nudge.hear(Ask::Take, Some((KIND, &dressing)));
        assert!(nudge.held.is_none());
    }

    /// **A step moves the number the handle names, and no other.**
    ///
    /// Three numbers under six keys is the whole of the mode question,
    /// and the way it goes wrong is quiet: an arrow that moves the
    /// rotation while the overlay says offset writes a number nobody
    /// meant into a file nobody re-reads.
    #[test]
    fn a_step_moves_the_number_the_handle_names_and_no_other() {
        let (mut nudge, dressing) = in_hand();
        let under = Some((KIND, &dressing));
        let step = |axis: usize, way: f32| Ask::Step {
            axis,
            way,
            fine: false,
        };
        nudge.hear(Ask::Hold(Handle::Move), under);
        for _ in 0..3 {
            nudge.hear(step(1, 1.0), under);
        }
        let held = nudge.held.as_ref().expect("in hand");
        assert_eq!(
            held.now.offset,
            Vec3::new(0.0, 0.15, 0.0),
            "three coarse moves are three coarse moves, and not 0.15000002"
        );
        assert_eq!(held.now.rotation, held.was.rotation);
        assert_eq!(held.now.scale, held.was.scale);
        assert!(!held.kept(), "a moved body still reads as the file's");

        nudge.hear(Ask::Hold(Handle::Turn), under);
        nudge.hear(step(1, 1.0), under);
        nudge.hear(Ask::Hold(Handle::Size), under);
        nudge.hear(step(0, -1.0), under);
        nudge.hear(
            Ask::Step {
                axis: 0,
                way: -1.0,
                fine: true,
            },
            under,
        );
        let held = nudge.held.as_ref().expect("in hand");
        assert_eq!(held.now.rotation, Vec3::new(0.0, 15.0, 0.0));
        assert_eq!(held.now.scale, Vec3::new(1.945, 2.0, 2.0));
        assert_eq!(held.now.offset, Vec3::new(0.0, 0.15, 0.0));

        // **A body cannot be nudged out of existence.** Scale is the one
        // number with a floor, because a mesh at zero has no volume to
        // find it by and getting it back would mean editing the file by
        // hand — which is the loop this exists to end.
        for _ in 0..200 {
            nudge.hear(step(2, -1.0), under);
        }
        let held = nudge.held.as_ref().expect("in hand");
        assert!(
            held.now.scale.z >= SIZE_FLOOR,
            "{:?} is not a body",
            held.now.scale
        );
    }

    /// **Putting it back is one key, and the file is another.**
    ///
    /// The undo goes back to what the FILE says rather than to where the
    /// hand started, which is the same thing until something is saved
    /// and deliberately not afterwards: once numbers are on disk, they
    /// are what "back" means.
    #[test]
    fn putting_it_back_is_one_key_and_writing_it_down_is_another() {
        let (mut nudge, dressing) = in_hand();
        let under = Some((KIND, &dressing));
        let up = Ask::Step {
            axis: 1,
            way: 1.0,
            fine: false,
        };
        assert!(
            !nudge.hear(Ask::Save, under),
            "a body nobody moved asked for a write"
        );
        nudge.hear(up, under);
        assert!(
            nudge.hear(Ask::Save, under),
            "a moved body asked for nothing"
        );
        // Until the caller says the file took it, the mark stays broken:
        // a write that failed must not read as a write that worked.
        assert!(!nudge.held.as_ref().expect("in hand").kept());
        nudge.keep();
        let held = nudge.held.as_ref().expect("in hand");
        assert!(held.kept());
        assert_eq!(held.was.offset, Vec3::new(0.0, 0.05, 0.0));

        nudge.hear(up, under);
        nudge.hear(Ask::Undo, under);
        let held = nudge.held.as_ref().expect("in hand");
        assert_eq!(
            held.now.offset,
            Vec3::new(0.0, 0.05, 0.0),
            "the undo went past what the file says"
        );
        assert!(held.kept());
    }

    /// **The overlay is told by shape, and never by hue.**
    ///
    /// The zero-text law's other half: what is drawn says what it means
    /// by form. The strongest version of that available is structural —
    /// [`Mark`] carries no colour at all, so there is no hue for a
    /// reading to hide in — and on top of it the three handles and the
    /// two states have to draw genuinely different pictures.
    #[test]
    fn the_overlay_is_told_by_shape_and_never_by_hue() {
        assert!(
            overlay(&Nudge::default()).is_empty(),
            "a bench holding nothing drew something"
        );
        let dressing = declared();
        let mut nudge = Nudge::default();
        nudge.hear(Ask::Take, Some((KIND, &dressing)));
        let mut pictures: Vec<(Handle, Vec<Mark>)> = Vec::new();
        for handle in [Handle::Move, Handle::Turn, Handle::Size] {
            nudge.hear(Ask::Hold(handle), None);
            let marks = overlay(&nudge);
            assert!(!marks.is_empty(), "{handle:?} drew nothing");
            pictures.push((handle, marks));
        }
        for (i, (handle, marks)) in pictures.iter().enumerate() {
            for (other, theirs) in pictures.iter().skip(i + 1) {
                assert_ne!(
                    marks, theirs,
                    "{handle:?} and {other:?} draw the same picture"
                );
            }
        }
        // Every handle says which way is plus, and says it the same way.
        let (mid, half) = Dressing::berth_box(KIND);
        for (handle, marks) in &pictures {
            for axis in 0..3 {
                let mut tip = mid;
                tip[axis] = half[axis].mul_add(PROUD, tip[axis]);
                assert!(
                    marks.iter().any(|mark| mark.at.translation == tip),
                    "{handle:?} leaves the plus end of axis {axis} unsaid"
                );
            }
        }
        // And the saved mark differs from the unsaved one by its shape,
        // not by anything a colourblind reader would miss.
        let kept = overlay(&nudge);
        nudge.hear(
            Ask::Step {
                axis: 0,
                way: 1.0,
                fine: false,
            },
            None,
        );
        let moved = overlay(&nudge);
        assert_ne!(
            kept.len(),
            moved.len(),
            "a body with unwritten numbers draws the same ring as one without"
        );
    }

    /// **The overlay stands clear of the body it is about**, on every
    /// kind and in every handle.
    ///
    /// An overlay drawn inside the mesh is an overlay nobody can see,
    /// and it is exactly the failure a bench cannot afford: the tool for
    /// putting a body where it belongs, hidden inside the body. Every
    /// berth in the game is asked, because the boxes are not all cubes —
    /// a `1×2` kind is twice as tall as it is wide, and a mark sized off
    /// the wrong half of it lands inside.
    ///
    /// The ring shape is asked here too, and it is not a nicety: a hoop
    /// takes an inner and an outer radius, so the spelling that looks
    /// right — a thin `inner`, an `outer` at the radius — is a doughnut
    /// filling its own middle, which is what this drew first.
    #[test]
    fn the_overlay_stands_clear_of_the_body_it_is_about() {
        for kind in Kind::ALL {
            let (mid, half) = Dressing::berth_box(kind);
            let numbers = Adjust {
                offset: Vec3::ZERO,
                rotation: Vec3::ZERO,
                scale: Vec3::ONE,
            };
            for handle in [Handle::Move, Handle::Turn, Handle::Size] {
                for kept in [true, false] {
                    let nudge = Nudge {
                        handle,
                        held: Some(Held {
                            id: String::new(),
                            kind,
                            was: numbers,
                            now: if kept {
                                numbers
                            } else {
                                Adjust {
                                    offset: Vec3::Y,
                                    ..numbers
                                }
                            },
                        }),
                    };
                    let marks = overlay(&nudge);
                    // The axis-aligned box each mark actually occupies,
                    // turn and all — the same reading `fill_box` takes.
                    let box_of = |mark: &Mark| {
                        let h = mark.body.half();
                        let m = Mat3::from_quat(mark.at.rotation);
                        let reach =
                            m.x_axis.abs() * h.x + m.y_axis.abs() * h.y + m.z_axis.abs() * h.z;
                        (mark.at.translation - reach, mark.at.translation + reach)
                    };
                    for axis in 0..3 {
                        let out = marks
                            .iter()
                            .map(|mark| box_of(mark).1[axis])
                            .fold(f32::NEG_INFINITY, f32::max);
                        assert!(
                            out > mid[axis] + half[axis],
                            "{kind:?} {handle:?}: nothing of the overlay stands outside                              the berth on axis {axis}"
                        );
                    }
                    // And the one mark that is not about an axis stands
                    // over the body rather than in it.
                    assert!(
                        marks.iter().any(|mark| box_of(mark).0.y > mid.y + half.y),
                        "{kind:?} {handle:?} (kept {kept}): the saved mark is inside the body"
                    );
                    for mark in &marks {
                        if let Body::Hoop { inner, outer } = mark.body {
                            let tube = (outer - inner) * 0.5;
                            assert!(
                                inner > 0.0 && tube < f32::midpoint(inner, outer) * 0.25,
                                "{kind:?} {handle:?}: {inner}..{outer} is a doughnut, not a ring"
                            );
                        }
                    }
                }
            }
        }
    }

    /// **The numbers the hands made are the numbers the file
    /// describes.**
    ///
    /// The seam, proved end to end and in one direction only: what the
    /// owner was looking at when they pressed the key is what the mesh
    /// will be standing at when the file is read back. It goes out
    /// through the writer and comes back through **the loader's own
    /// parse** — never a second reader written for the test — and what
    /// is compared is the pose, because the pose is the thing the owner
    /// was actually judging.
    #[test]
    fn the_numbers_the_hands_made_are_the_numbers_the_file_describes() {
        let (mut nudge, dressing) = in_hand();
        let under = Some((KIND, &dressing));
        let step = |axis: usize, way: f32, fine: bool| Ask::Step { axis, way, fine };
        nudge.hear(Ask::Hold(Handle::Move), under);
        nudge.hear(step(1, -1.0, false), under);
        nudge.hear(step(2, 1.0, true), under);
        nudge.hear(Ask::Hold(Handle::Turn), under);
        for _ in 0..6 {
            nudge.hear(step(1, 1.0, false), under);
        }
        nudge.hear(Ask::Hold(Handle::Size), under);
        nudge.hear(step(0, -1.0, false), under);
        let held = nudge.held.clone().expect("in hand");
        let seen = held.now.pose(&dressing, KIND);

        let text = crate::art::rewritten(DECLARED, &held.id, &held.now.lines())
            .expect("the fixture carries the table");
        let read = crate::art::Dressings::read(&text)
            .expect("what the writer wrote, the reader reads")
            .of(KIND)
            .expect("still dressed")
            .clone();
        assert_eq!(read.offset, held.now.offset);
        assert_eq!(read.rotation, held.now.rotation);
        assert_eq!(read.scale, held.now.scale);
        // Untouched, and deliberately: `fill` is the promise the mesh is
        // measured against, and a bench that wrote it would make it
        // unbreakable.
        assert_eq!(read.fill, dressing.fill);
        assert_eq!(read.measured, dressing.measured);
        let filed = read.pose(KIND);
        assert_eq!(
            filed, seen,
            "the mesh the owner saw is not the mesh the file describes"
        );
        // A quarter turn was asked for, and a quarter turn is what the
        // file says — the frame the writer emits in is the frame the
        // loader reads in, on the axis it is easiest to get wrong.
        assert!(
            (filed.rotation * Vec3::Z - Vec3::X).length() < 1e-5,
            "{:?}",
            filed.rotation
        );
    }

    /// **The bench speaks only in shapes.** The menu's own guard, asked
    /// of the other thing in this cabin that draws a reading.
    #[test]
    fn the_bench_speaks_only_in_shapes() {
        let source = include_str!("nudge.rs");
        // Spelled in pieces so the test does not trip over itself.
        let text_node = ["Text", "::new"].concat();
        assert!(
            !source.contains(&text_node),
            "the bench grew a rendered string; shapes only (DESIGN.md)"
        );
    }

    /// **Nothing but the bench writes the manifest.**
    ///
    /// The gate is that `--nudge` is the only thing that installs a
    /// system which can open the file. That is worth a ratchet, because
    /// the cheapest way to break it is for some future convenience to
    /// call the writer from a system that is always on. The writer lives
    /// in `art`, and the only other file in the crate allowed to name it
    /// is this one.
    #[test]
    fn nothing_but_the_bench_writes_the_manifest() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut callers: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&src).expect("the source directory") {
            let path = entry.expect("an entry").path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source readable");
            if text.contains("save_into(") {
                callers.push(
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
        }
        callers.sort();
        assert_eq!(
            callers,
            vec!["art.rs".to_owned(), "nudge.rs".to_owned()],
            "something other than the bench can write the owner's manifest"
        );
    }
}

/// **The bench, driven the way a player drives it**: real key edges over
/// a real schedule, against a real sim, with no window and no mesh.
///
/// The cabin's own scripted sessions (`crate::session`) are the model
/// and the mechanism is theirs: a script holds keys down, a system
/// inside the schedule turns that into this frame's edges — because
/// Bevy clears last frame's before `Update` and a press written from
/// outside is a press nobody sees — and the systems under test are the
/// ones `plugin` installs, in the order it installs them.
///
/// What that buys is the whole of the bench except its pixels: the
/// vocabulary, the state machine, the live pose, the write, the read
/// back, and the overlay's own entities. What it does not and cannot
/// buy is whether any of it is legible, which needs eyes and is
/// recorded as such.
#[cfg(all(test, feature = "art"))]
mod scripted {
    use bevy::input::InputPlugin;
    use bevy::prelude::*;
    use space_trucking::sim::{Kind, Sim, Vec2 as SimVec2, layout};

    use super::Nudge;
    use super::bench::{Bench, BenchMark, Drawn, draw, hands, wear};
    use crate::art::{Dressings, Worn};
    use crate::bridge::{Bridge, FrameOutcome};
    use crate::surface::VirtualPointer;
    use crate::{Phase, Shell};

    /// The keys the script is holding down.
    #[derive(Resource, Default)]
    struct Script(Vec<KeyCode>);

    /// Turn the script's keys into this frame's edges, after Bevy's own
    /// input pass has cleared last frame's.
    fn press(script: Res<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
        let down: Vec<KeyCode> = keys.get_pressed().copied().collect();
        for key in down {
            if !script.0.contains(&key) {
                keys.release(key);
            }
        }
        for key in &script.0 {
            if !keys.pressed(*key) {
                keys.press(*key);
            }
        }
    }

    /// **A body on the board the crosshair can actually rest on**: its
    /// kind, and where the pointer has to be for the pick to answer it.
    ///
    /// Searched rather than written down, for the reason the session
    /// harness searches for its own spots — the fixture is re-dressed
    /// whenever the cargo tables change, and a coordinate spelled out
    /// here would quietly stop being the coordinate.
    fn a_body(sim: &Sim) -> (Kind, SimVec2) {
        for piece in sim.pieces() {
            let rect = layout::piece_rect(sim.rooms(), sim.pieces(), piece);
            let at = SimVec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y));
            if layout::piece_at(sim.rooms(), sim.pieces(), at).map(|found| found.id)
                == Some(piece.id)
            {
                return (piece.kind, at);
            }
        }
        panic!("the fixture board has no body the crosshair can rest on");
    }

    /// A bench standing in a scratch directory, with one kind dressed.
    struct Stand {
        app: App,
        dir: std::path::PathBuf,
        kind: Kind,
    }

    impl Stand {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(name);
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("a scratch bench");

            let mut bridge = Bridge::boot_fixture(crate::fixture::SAVE);
            bridge.steady();
            let (kind, at) = a_body(&bridge.sim);
            // The manifest and the index are one dialect, so the fixture
            // is one text: what the resolver would have written, and what
            // the owner would have typed, agreeing — which is the state a
            // bench is opened in.
            let declared = format!(
                "# A fixture manifest, with a word in it that must not move.\n\n\
                 [asset.bought]\nglb = \"glb/abc.glb\"\ndresses = \"cargo/{}\"\n\
                 scale = [1.0, 1.0, 1.0]\noffset = [0.0, 0.0, 0.0]\n\
                 fill = [1.0, 1.0, 1.0]\nmeasured_mid = [0.0, 0.0, 0.0]\n\
                 measured_half = [0.5, 0.5, 0.5]\n",
                crate::art::snake(kind)
            );
            for file in ["manifest.toml", "index.toml"] {
                std::fs::write(dir.join(file), &declared).expect("a fixture file");
            }

            let mut app = App::new();
            app.add_plugins((MinimalPlugins, InputPlugin))
                .insert_resource(Shell {
                    bridge,
                    outcome: FrameOutcome::default(),
                    muted: false,
                })
                .insert_resource(crate::menu::Menu::boot(false))
                .insert_resource(VirtualPointer {
                    sim: at,
                    ..VirtualPointer::default()
                })
                .insert_resource(
                    Dressings::read(&declared).expect("the dialect the resolver writes"),
                )
                .insert_resource(Bench {
                    manifest: dir.join("manifest.toml"),
                    index: dir.join("index.toml"),
                })
                .insert_resource(Assets::<Mesh>::default())
                .insert_resource(Assets::<StandardMaterial>::default())
                .init_resource::<Nudge>()
                .init_resource::<Drawn>()
                .init_resource::<Script>()
                .configure_sets(Update, (Phase::Input, Phase::View).chain())
                .add_systems(Update, press.in_set(Phase::Input))
                .add_systems(Update, (hands, wear, draw).chain().in_set(Phase::View));
            // One rig wearing the bought body, which is what
            // `pieces::build_kind` spawns under the feature.
            let root = app.world_mut().spawn(Transform::default()).id();
            app.world_mut()
                .spawn((Worn(kind), Transform::default(), ChildOf(root)));
            let mut stand = Self { app, dir, kind };
            stand.tap(&[]);
            stand
        }

        /// Hold these keys for a frame, then let go for another — one
        /// press, exactly as a hand makes one.
        fn tap(&mut self, keys: &[KeyCode]) {
            self.app.world_mut().resource_mut::<Script>().0 = keys.to_vec();
            self.app.update();
            self.app.world_mut().resource_mut::<Script>().0.clear();
            self.app.update();
        }

        fn nudge(&self) -> &Nudge {
            self.app.world().resource::<Nudge>()
        }

        /// Where the bought body is standing this frame.
        fn worn(&mut self) -> Transform {
            *self
                .app
                .world_mut()
                .query_filtered::<&Transform, With<Worn>>()
                .iter(self.app.world())
                .next()
                .expect("one bought body")
        }

        /// How many marks the overlay has on screen.
        fn marks(&mut self) -> usize {
            self.app
                .world_mut()
                .query_filtered::<Entity, With<BenchMark>>()
                .iter(self.app.world())
                .count()
        }

        /// What the file says now, through the loader's own parse.
        fn filed(&self) -> crate::art::Dressing {
            let text =
                std::fs::read_to_string(self.dir.join("manifest.toml")).expect("still readable");
            Dressings::read(&text)
                .expect("what the bench wrote, the loader reads")
                .of(self.kind)
                .expect("still dressed")
                .clone()
        }
    }

    impl Drop for Stand {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// **A scripted session takes a body, nudges it, and writes the
    /// file** — and what the file then says is where the body is
    /// standing.
    ///
    /// The whole loop the bench exists for, run headless: the pick, the
    /// take, three steps, the write, the read back, and the letting go.
    #[test]
    fn a_scripted_session_takes_a_body_nudges_it_and_writes_the_file() {
        let mut bench = Stand::new("space-trucking-nudge-session");
        assert_eq!(bench.marks(), 0, "a bench holding nothing drew something");

        bench.tap(&[KeyCode::Tab]);
        let held = bench
            .nudge()
            .held
            .clone()
            .expect("the crosshair took a body");
        assert_eq!(held.id, "bought", "the table a save will write");
        assert!(bench.marks() > 0, "a body in hand drew no overlay");

        // Three steps up the berth's own y, through real key edges.
        for _ in 0..3 {
            bench.tap(&[KeyCode::ArrowUp]);
        }
        let now = bench.nudge().held.as_ref().expect("in hand").now;
        assert_eq!(now.offset, Vec3::new(0.0, 0.15, 0.0));
        let seen = bench.worn();
        assert_eq!(
            seen,
            now.pose(&bench.filed(), bench.kind),
            "the body in the room is not wearing the numbers in hand"
        );

        bench.tap(&[KeyCode::Enter]);
        let filed = bench.filed();
        assert_eq!(filed.offset, Vec3::new(0.0, 0.15, 0.0));
        assert_eq!(
            filed.pose(bench.kind),
            seen,
            "the mesh the owner saw is not the mesh the file describes"
        );
        // The prose in the fixture is still there, and so is the promise
        // the bench does not write.
        let text = std::fs::read_to_string(bench.dir.join("manifest.toml")).expect("readable");
        assert!(text.contains("must not move"), "{text}");
        assert!(text.contains("fill = [1.0, 1.0, 1.0]"), "{text}");
        // And the derived file the resolver owns carries it too, so a
        // restart shows what was saved.
        let index = std::fs::read_to_string(bench.dir.join("index.toml")).expect("readable");
        assert!(index.contains("offset = [0.0, 0.15, 0.0]"), "{index}");

        // Letting go leaves the body where it was written, because what
        // this run believes moved with the file — and takes the overlay
        // off the screen with it.
        bench.tap(&[KeyCode::Tab]);
        assert!(bench.nudge().held.is_none(), "the body would not be let go");
        assert_eq!(bench.worn(), seen, "a saved body sprang back on release");
        assert_eq!(bench.marks(), 0, "the overlay outlived the hand");
    }

    /// **An unsaved body goes back where the file says when it is let
    /// go**, and the menu takes the keyboard away while it stands.
    ///
    /// The other half of the previous guard: a nudge nobody wrote down
    /// is a nudge that leaves nothing behind, which is what makes trying
    /// something cheap.
    #[test]
    fn an_unwritten_nudge_leaves_nothing_behind() {
        let mut bench = Stand::new("space-trucking-nudge-undo");
        let rest = bench.worn();
        bench.tap(&[KeyCode::Tab]);
        bench.tap(&[KeyCode::ArrowRight]);
        let moved = bench.worn();
        assert_ne!(moved, rest, "the arrow moved nothing");

        // While the menu stands it owns the keyboard, exactly as it does
        // for the camera: the save key does not reach the file.
        bench
            .app
            .world_mut()
            .resource_mut::<crate::menu::Menu>()
            .open = true;
        bench.tap(&[KeyCode::Enter]);
        assert_eq!(
            bench.filed().offset,
            Vec3::ZERO,
            "a key the menu was holding reached the file"
        );
        bench
            .app
            .world_mut()
            .resource_mut::<crate::menu::Menu>()
            .open = false;

        // Put it back with one key, and let go: nothing moved, nothing
        // written.
        bench.tap(&[KeyCode::Backspace]);
        assert_eq!(bench.worn(), rest, "the undo did not put the body back");
        bench.tap(&[KeyCode::Tab]);
        assert_eq!(bench.worn(), rest);
        assert_eq!(bench.filed().offset, Vec3::ZERO);
        assert_eq!(
            std::fs::read_to_string(bench.dir.join("manifest.toml"))
                .expect("readable")
                .matches("offset")
                .count(),
            1,
            "a session that saved nothing changed the file"
        );
    }

    /// **A body nobody dressed cannot be taken at all**, so the bench in
    /// a cabin with no art in it is a bench that does nothing.
    #[test]
    fn a_body_nothing_dresses_cannot_be_taken() {
        let mut bench = Stand::new("space-trucking-nudge-bare");
        *bench.app.world_mut().resource_mut::<Dressings>() = Dressings::default();
        bench.tap(&[KeyCode::Tab]);
        assert!(bench.nudge().held.is_none());
        assert_eq!(bench.marks(), 0);
        bench.tap(&[KeyCode::Enter]);
        assert!(
            std::fs::read_to_string(bench.dir.join("manifest.toml"))
                .expect("readable")
                .contains("offset = [0.0, 0.0, 0.0]"),
            "a bench with nothing in hand wrote to the file"
        );
    }
}
