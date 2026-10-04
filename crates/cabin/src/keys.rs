//! **The carry's keys: one table, read wherever a carry key is read, and
//! rebound on the `Esc` menu's keys page.**
//!
//! The owner, after a playtest: "Q + Shift Q feel weird, maybe Q + E
//! instead? Ctrl felt fine. Maybe time to add a keybind menu option to
//! this one." So what a key does to a carry — turn it either way, raise
//! it, lower it — is an [`Action`], each action is bound to exactly one
//! key in one table ([`Bindings`]), and the defaults are `Q` and `E` for
//! the turns and `X` and `Z` for the lift (docs/BAY.md, "Lift, and the
//! keys"). Nothing reads a carry key by its `KeyCode` but this table:
//! `main::hands` asks it which actions were pressed, `rig::steer` asks it
//! whether `E` belongs to a carry before it focuses with one, and the
//! menu's keys page rebinds it (`crate::menu`).
//!
//! **The table is a list of actions, not four fields**, so the next thing
//! a key does joins it as a row: a token for the file, a default, and a
//! glyph on the page. The wheel's gestures — the turn, `Ctrl`'s fine
//! turn, `Shift`'s lift — are not in it: they are a wheel and two
//! modifiers, not keys, and this pass rebinds keys.
//!
//! **Two laws, and [`Bindings::bind`] is the only door a key comes in by.**
//! No action is ever left unbound and no key does two things: binding a
//! key another action holds SWAPS the two. And a key the game already
//! answers is refused ([`refusal`]), because a carry key that also
//! walked, paused or opened the menu would do two things whichever
//! action it was.
//!
//! **Kept beside the save** (`cabin.keys`, in the working directory with
//! `cabin.data`), in a line format like the save's: a header, then one
//! `action key` line per action. A missing or unreadable file is the
//! defaults; a malformed line is skipped and its action keeps its
//! default, never a failed boot. A browser build has no working
//! directory, so there the bindings last the session.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

/// **The file the bindings are kept in**, beside the save.
const KEYS_FILE: &str = "cabin.keys";

/// The file's first line. A file that opens with anything else is not a
/// bindings file this build reads, and the defaults stand.
const MAGIC: &str = "KEYS1";

/// **What a key may do to a carry.** Declaration order is the page's
/// order and the file's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// A fifteen-degree step counter-clockwise, as seen from the room:
    /// the way the wheel rolled up turns.
    TurnCcw,
    /// The same step clockwise.
    TurnCw,
    /// A sixteenth of a cell away from the surface.
    Raise,
    /// A sixteenth of a cell back toward it.
    Lower,
}

impl Action {
    /// Every action, in the page's order.
    pub const ALL: [Self; 4] = [Self::TurnCcw, Self::TurnCw, Self::Raise, Self::Lower];

    /// **The default key**, which the page's reset puts back.
    ///
    /// `Q` and `E` for the turns, under the fingers that walk, as the
    /// owner asked; `E` is focus too, and which one it is depends on the
    /// hand (`rig::steer`). `X` and `Z` for the lift, on the row under
    /// the walk where the same hand already is, `X` up and `Z` down. None
    /// is a key the game or the bench answers, and `R` stays nobody's.
    #[must_use]
    pub const fn default_key(self) -> KeyCode {
        match self {
            Self::TurnCcw => KeyCode::KeyQ,
            Self::TurnCw => KeyCode::KeyE,
            Self::Raise => KeyCode::KeyX,
            Self::Lower => KeyCode::KeyZ,
        }
    }

    /// The word the file writes the action under. The file is not the
    /// game's text: nothing renders it.
    const fn token(self) -> &'static str {
        match self {
            Self::TurnCcw => "turn-ccw",
            Self::TurnCw => "turn-cw",
            Self::Raise => "raise",
            Self::Lower => "lower",
        }
    }

    /// The action written as `token`, if any.
    fn of_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.token() == token)
    }

    /// This action's row in the table.
    const fn row(self) -> usize {
        self as usize
    }
}

/// **Why a key may not be bound**, said by the page as a refusal on the
/// keycap and by the file reader by skipping the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The game answers it already: `Esc`, the walk, a toggle, a
    /// modifier the carry or the click reads, or the system's own.
    Game,
    /// The placement bench answers it, under `--nudge`.
    Bench,
    /// The page has no name to print on its keycap.
    Unnamed,
}

/// **The keys the game itself answers**, which no carry action may take.
///
/// - `Esc` opens the menu, steps out of a station and cancels a capture;
///   a carry key on it would do one of those as well.
/// - `W`, `A`, `S`, `D` walk, and a carry is walked.
/// - `Space`, `F` and `M` are pause, warp and mute, live in every mode.
/// - `Shift` and `Ctrl` are read with the wheel and the click — the lift,
///   the fine turn, quick-move — so a step bound to one would fire under
///   every one of those gestures.
/// - `Alt` and `Super` are the system's: window switching, the OS menu.
///   `Super` parks the cursor (`rig::steer`).
const GAME: [KeyCode; 16] = [
    KeyCode::Escape,
    KeyCode::KeyW,
    KeyCode::KeyA,
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::Space,
    KeyCode::KeyF,
    KeyCode::KeyM,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::SuperLeft,
    KeyCode::SuperRight,
];

/// **Whether `key` may be bound, and why not.** The game's own keys
/// first, then the bench's, then whether the page can name it.
///
/// **The bench's keys are refused in every build**, not only under
/// `--nudge`. The bindings are kept in a file every build reads, so a key
/// bound in a played session is bound in the bench's session too, and the
/// bench's own test (`nudge::tests::the_benchs_keys_are_the_benchs_alone`)
/// can promise the bench its keys only if no binding can ever take them.
/// That costs the arrows, the brackets, `T`, `R`, `G`, `Tab`, `Enter` and
/// `Backspace`, and keeps `R` — which once threw a run away — nobody's
/// (`docs/DESIGN_REVIEW.md`).
#[must_use]
pub fn refusal(key: KeyCode) -> Option<Refused> {
    if GAME.contains(&key) {
        Some(Refused::Game)
    } else if crate::nudge::bench_keys().contains(&key) {
        Some(Refused::Bench)
    } else if name(key).is_none() {
        Some(Refused::Unnamed)
    } else {
        None
    }
}

/// **Every key a binding may take, with the word the file writes it as
/// and the name its keycap prints.**
///
/// A closed list rather than whatever the keyboard sends, because a key
/// is shown on the page by its name and a name is text: each one here is
/// short, plain ASCII the menu's font has. A `KeyCode` is a PLACE on the
/// keyboard, not the letter printed on it, so a name is the one a US
/// board prints at that place — the place the game reads, on every
/// layout. None of them is a key [`refusal`] keeps.
const NAMED: &[(KeyCode, &str, &str)] = &[
    (KeyCode::KeyB, "KeyB", "B"),
    (KeyCode::KeyC, "KeyC", "C"),
    (KeyCode::KeyE, "KeyE", "E"),
    (KeyCode::KeyH, "KeyH", "H"),
    (KeyCode::KeyI, "KeyI", "I"),
    (KeyCode::KeyJ, "KeyJ", "J"),
    (KeyCode::KeyK, "KeyK", "K"),
    (KeyCode::KeyL, "KeyL", "L"),
    (KeyCode::KeyN, "KeyN", "N"),
    (KeyCode::KeyO, "KeyO", "O"),
    (KeyCode::KeyP, "KeyP", "P"),
    (KeyCode::KeyQ, "KeyQ", "Q"),
    (KeyCode::KeyU, "KeyU", "U"),
    (KeyCode::KeyV, "KeyV", "V"),
    (KeyCode::KeyX, "KeyX", "X"),
    (KeyCode::KeyY, "KeyY", "Y"),
    (KeyCode::KeyZ, "KeyZ", "Z"),
    (KeyCode::Digit0, "Digit0", "0"),
    (KeyCode::Digit1, "Digit1", "1"),
    (KeyCode::Digit2, "Digit2", "2"),
    (KeyCode::Digit3, "Digit3", "3"),
    (KeyCode::Digit4, "Digit4", "4"),
    (KeyCode::Digit5, "Digit5", "5"),
    (KeyCode::Digit6, "Digit6", "6"),
    (KeyCode::Digit7, "Digit7", "7"),
    (KeyCode::Digit8, "Digit8", "8"),
    (KeyCode::Digit9, "Digit9", "9"),
    (KeyCode::F1, "F1", "F1"),
    (KeyCode::F2, "F2", "F2"),
    (KeyCode::F3, "F3", "F3"),
    (KeyCode::F4, "F4", "F4"),
    (KeyCode::F5, "F5", "F5"),
    (KeyCode::F6, "F6", "F6"),
    (KeyCode::F7, "F7", "F7"),
    (KeyCode::F8, "F8", "F8"),
    (KeyCode::F9, "F9", "F9"),
    (KeyCode::F10, "F10", "F10"),
    (KeyCode::F11, "F11", "F11"),
    (KeyCode::F12, "F12", "F12"),
    (KeyCode::Minus, "Minus", "-"),
    (KeyCode::Equal, "Equal", "="),
    (KeyCode::Backslash, "Backslash", "\\"),
    (KeyCode::Semicolon, "Semicolon", ";"),
    (KeyCode::Quote, "Quote", "'"),
    (KeyCode::Comma, "Comma", ","),
    (KeyCode::Period, "Period", "."),
    (KeyCode::Slash, "Slash", "/"),
    (KeyCode::Backquote, "Backquote", "`"),
    (KeyCode::Insert, "Insert", "Ins"),
    (KeyCode::Delete, "Delete", "Del"),
    (KeyCode::Home, "Home", "Home"),
    (KeyCode::End, "End", "End"),
    (KeyCode::PageUp, "PageUp", "PgUp"),
    (KeyCode::PageDown, "PageDown", "PgDn"),
    (KeyCode::Numpad0, "Numpad0", "Num0"),
    (KeyCode::Numpad1, "Numpad1", "Num1"),
    (KeyCode::Numpad2, "Numpad2", "Num2"),
    (KeyCode::Numpad3, "Numpad3", "Num3"),
    (KeyCode::Numpad4, "Numpad4", "Num4"),
    (KeyCode::Numpad5, "Numpad5", "Num5"),
    (KeyCode::Numpad6, "Numpad6", "Num6"),
    (KeyCode::Numpad7, "Numpad7", "Num7"),
    (KeyCode::Numpad8, "Numpad8", "Num8"),
    (KeyCode::Numpad9, "Numpad9", "Num9"),
    (KeyCode::NumpadAdd, "NumpadAdd", "Num+"),
    (KeyCode::NumpadSubtract, "NumpadSubtract", "Num-"),
    (KeyCode::NumpadMultiply, "NumpadMultiply", "Num*"),
    (KeyCode::NumpadDivide, "NumpadDivide", "Num/"),
    (KeyCode::NumpadDecimal, "NumpadDecimal", "Num."),
];

/// **The name `key`'s keycap prints**, if the page can name it.
#[must_use]
pub fn name(key: KeyCode) -> Option<&'static str> {
    NAMED
        .iter()
        .find(|(named, _, _)| *named == key)
        .map(|(_, _, name)| *name)
}

/// The word the file writes `key` as.
fn token(key: KeyCode) -> Option<&'static str> {
    NAMED
        .iter()
        .find(|(named, _, _)| *named == key)
        .map(|(_, token, _)| *token)
}

/// The key the file wrote as `token`.
fn of_token(token: &str) -> Option<KeyCode> {
    NAMED
        .iter()
        .find(|(_, named, _)| *named == token)
        .map(|(key, _, _)| *key)
}

/// **The table**: one key per action, and where it is kept.
#[derive(Resource, Clone, Debug)]
pub struct Bindings {
    keys: [KeyCode; Action::ALL.len()],
    /// The file the table is written to whenever it changes, and read
    /// from at boot: beside the save on a native build, nowhere in a
    /// browser or a test.
    file: Option<PathBuf>,
}

impl Default for Bindings {
    /// The defaults, kept nowhere.
    fn default() -> Self {
        Self {
            keys: Action::ALL.map(Action::default_key),
            file: None,
        }
    }
}

impl Bindings {
    /// **The player's bindings, as the boot finds them**: read from the
    /// file beside the save, and written back there whenever the page
    /// changes them. A browser build has no file and keeps the defaults
    /// for the session.
    #[must_use]
    pub fn boot() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::default()
        } else {
            Self::kept_at(crate::bridge::save_path(KEYS_FILE))
        }
    }

    /// The bindings kept in `file`: what it says, or the defaults where it
    /// says nothing this build reads.
    #[must_use]
    pub fn kept_at(file: PathBuf) -> Self {
        let mut bindings = std::fs::read_to_string(&file)
            .map(|text| Self::read(&text))
            .unwrap_or_default();
        bindings.file = Some(file);
        bindings
    }

    /// **The table a file's text describes.** The first line must be the
    /// header, or none of it is read. After it, each line that names an
    /// action and a key the page may bind binds it, through the very
    /// [`Bindings::bind`] the page uses — so a hand-edited file that gives
    /// two actions one key swaps them as a click would, and no key ever
    /// does two things. Anything else on a line, or a line that is not one
    /// of those, is skipped, and the action keeps the key it had.
    fn read(text: &str) -> Self {
        let mut bindings = Self::default();
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(MAGIC) {
            return bindings;
        }
        for line in lines {
            let mut words = line.split_whitespace();
            let (Some(action), Some(key), None) = (words.next(), words.next(), words.next()) else {
                continue;
            };
            if let (Some(action), Some(key)) = (Action::of_token(action), of_token(key)) {
                let _ = bindings.set(action, key);
            }
        }
        bindings
    }

    /// The file's text for this table.
    fn text(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail.
        let _ = writeln!(out, "{MAGIC}");
        for action in Action::ALL {
            // Every bound key is a named one: `bind` refuses the rest.
            if let Some(key) = token(self.key(action)) {
                let _ = writeln!(out, "{} {key}", action.token());
            }
        }
        out
    }

    /// **The key `action` is bound to.**
    #[must_use]
    pub const fn key(&self, action: Action) -> KeyCode {
        self.keys[action.row()]
    }

    /// The action `key` is bound to, if any.
    #[must_use]
    pub fn action(&self, key: KeyCode) -> Option<Action> {
        Action::ALL
            .into_iter()
            .find(|action| self.key(*action) == key)
    }

    /// **Whether `action`'s key went down this frame.** An edge: one press
    /// is one step, as the nudge bench's are, because a held key at sixty
    /// steps a second is a key no hand can stop where it wanted.
    #[must_use]
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        keys.just_pressed(self.key(action))
    }

    /// **Bind `key` to `action`**, and keep the table where it is kept.
    ///
    /// A key another action holds is SWAPPED: that action takes the key
    /// `action` had, so no action is ever left unbound and no key does two
    /// things, and the answer is that other action, so the page can say
    /// it moved. A key [`refusal`] keeps is refused and the table is
    /// untouched.
    pub fn bind(&mut self, action: Action, key: KeyCode) -> Result<Option<Action>, Refused> {
        let swapped = self.set(action, key)?;
        self.keep();
        Ok(swapped)
    }

    /// [`Bindings::bind`] without the keeping, for the reader.
    fn set(&mut self, action: Action, key: KeyCode) -> Result<Option<Action>, Refused> {
        if let Some(refused) = refusal(key) {
            return Err(refused);
        }
        let had = self.key(action);
        let swapped = self.action(key).filter(|other| *other != action);
        if let Some(other) = swapped {
            self.keys[other.row()] = had;
        }
        self.keys[action.row()] = key;
        Ok(swapped)
    }

    /// **Every action back on its default key**, kept.
    pub fn reset(&mut self) {
        self.keys = Action::ALL.map(Action::default_key);
        self.keep();
    }

    /// Whether every action is on its default key.
    #[must_use]
    pub fn defaults(&self) -> bool {
        Action::ALL
            .into_iter()
            .all(|action| self.key(action) == action.default_key())
    }

    /// Write the table to its file, where it has one. Failure is silent,
    /// as the save's is: a read-only directory costs the bindings their
    /// next boot, not the session.
    fn keep(&self) {
        if let Some(file) = &self.file {
            store(file, &self.text());
        }
    }
}

/// Write `text` to `file`, quietly.
fn store(file: &Path, text: &str) {
    let _ = std::fs::write(file, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch file of this test's own, so no test writes the player's
    /// `cabin.keys` and no two tests share one.
    fn scratch(name: &str) -> PathBuf {
        let file =
            std::env::temp_dir().join(format!("space-trucking-keys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_file(&file);
        file
    }

    /// **The defaults are the owner's**, every one of them a key the page
    /// may bind, and no two the same.
    #[test]
    fn the_defaults_are_q_e_x_and_z() {
        let bindings = Bindings::default();
        assert_eq!(bindings.key(Action::TurnCcw), KeyCode::KeyQ);
        assert_eq!(bindings.key(Action::TurnCw), KeyCode::KeyE);
        assert_eq!(bindings.key(Action::Raise), KeyCode::KeyX);
        assert_eq!(bindings.key(Action::Lower), KeyCode::KeyZ);
        for action in Action::ALL {
            assert_eq!(refusal(action.default_key()), None, "{action:?}'s default");
        }
        assert!(bindings.defaults());
    }

    /// **Binding a held key swaps the two**, so no action is left unbound
    /// and no key does two things; binding a free key frees the old one.
    #[test]
    fn binding_a_held_key_swaps_the_two() {
        let mut bindings = Bindings::default();
        assert_eq!(
            bindings.bind(Action::Raise, KeyCode::KeyQ),
            Ok(Some(Action::TurnCcw))
        );
        assert_eq!(bindings.key(Action::Raise), KeyCode::KeyQ);
        assert_eq!(bindings.key(Action::TurnCcw), KeyCode::KeyX);
        assert_eq!(bindings.bind(Action::Raise, KeyCode::KeyC), Ok(None));
        assert_eq!(bindings.key(Action::Raise), KeyCode::KeyC);
        assert_eq!(bindings.action(KeyCode::KeyQ), None, "Q stayed bound");
        assert_eq!(bindings.bind(Action::Raise, KeyCode::KeyC), Ok(None));
        let mut keys: Vec<KeyCode> = Action::ALL.map(|action| bindings.key(action)).to_vec();
        keys.sort_by_key(|key| format!("{key:?}"));
        keys.dedup();
        assert_eq!(keys.len(), Action::ALL.len(), "a key does two things");
        bindings.reset();
        assert!(bindings.defaults());
    }

    /// **The game's keys and the bench's are refused, and so is a key
    /// the page cannot name**, and a refusal leaves the table as it was.
    #[test]
    fn a_reserved_key_is_refused() {
        let mut bindings = Bindings::default();
        for (key, why) in [
            (KeyCode::Escape, Refused::Game),
            (KeyCode::KeyW, Refused::Game),
            (KeyCode::Space, Refused::Game),
            (KeyCode::ShiftLeft, Refused::Game),
            (KeyCode::KeyR, Refused::Bench),
            (KeyCode::ArrowUp, Refused::Bench),
            (KeyCode::Tab, Refused::Bench),
            (KeyCode::CapsLock, Refused::Unnamed),
        ] {
            assert_eq!(bindings.bind(Action::Lower, key), Err(why), "{key:?}");
            assert!(bindings.defaults(), "a refusal moved a key");
        }
    }

    /// **Every name is short plain text, and every named key is one a
    /// binding may take.** A keycap is the only place the page prints
    /// words, and the menu's font has ASCII and nothing else.
    #[test]
    fn every_named_key_is_bindable_and_plain() {
        for &(key, token, name) in NAMED {
            assert!(
                !GAME.contains(&key) && !crate::nudge::bench_keys().contains(&key),
                "{token} is named and reserved"
            );
            assert!(
                !name.is_empty() && name.len() <= 4 && name.is_ascii(),
                "{token}'s keycap reads {name:?}"
            );
            assert_eq!(
                of_token(token),
                Some(key),
                "{token} reads back as another key"
            );
        }
        let mut tokens: Vec<&str> = NAMED.iter().map(|(_, token, _)| *token).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), NAMED.len(), "two keys share a token");
    }

    /// **The bindings survive the trip through their file**, and a file
    /// nobody wrote is the defaults.
    #[test]
    fn bindings_survive_their_file() {
        let file = scratch("trip");
        let fresh = Bindings::kept_at(file.clone());
        assert!(fresh.defaults(), "a missing file is the defaults");
        let mut bindings = fresh;
        assert_eq!(bindings.bind(Action::TurnCcw, KeyCode::KeyC), Ok(None));
        assert_eq!(
            bindings.bind(Action::Raise, KeyCode::KeyE),
            Ok(Some(Action::TurnCw))
        );
        let again = Bindings::kept_at(file.clone());
        for action in Action::ALL {
            assert_eq!(
                again.key(action),
                bindings.key(action),
                "{action:?} came back"
            );
        }
        let _ = std::fs::remove_file(&file);
    }

    /// **A malformed file is the defaults, line by line**: an unreadable
    /// header reads nothing, and after a good one every line that is not
    /// an action and a bindable key is skipped while the rest are read. A
    /// file that gives two actions one key swaps them, as the page would.
    #[test]
    fn a_malformed_file_falls_back_to_the_defaults() {
        let file = scratch("bad");
        for text in ["", "KEYS0\nraise KeyC\n", "raise KeyC\n", "\u{1F680}"] {
            store(&file, text);
            assert!(
                Bindings::kept_at(file.clone()).defaults(),
                "{text:?} was read"
            );
        }
        store(
            &file,
            "KEYS1\nraise KeyC extra\nturn-ccw\nlower KeyW\nfly KeyB\nturn-cw KeyCapsLock\n\
             raise KeyR\nlower KeyN\n",
        );
        let read = Bindings::kept_at(file.clone());
        assert_eq!(
            read.key(Action::Lower),
            KeyCode::KeyN,
            "the good line was lost"
        );
        for action in [Action::TurnCcw, Action::TurnCw, Action::Raise] {
            assert_eq!(
                read.key(action),
                action.default_key(),
                "{action:?} read a bad line"
            );
        }
        store(&file, "KEYS1\nraise KeyQ\n");
        let read = Bindings::kept_at(file.clone());
        assert_eq!(read.key(Action::Raise), KeyCode::KeyQ);
        assert_eq!(
            read.key(Action::TurnCcw),
            KeyCode::KeyX,
            "a file made a key do two things"
        );
        let _ = std::fs::remove_file(&file);
    }
}
