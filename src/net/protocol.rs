//! The wire protocol: versioned, line-oriented lockstep messages.
//!
//! Style and defenses mirror `sim/save.rs`: a magic-plus-version header
//! (`SNP2`), whitespace-separated tokens, and parsing that never panics —
//! every malformed message maps to a [`WireError`] with its 1-based line
//! number (line 0 means the text ended too early). Floats travel as hex bit
//! patterns, never decimal, because a pointer position that drifts by one
//! ULP on the wire would fork the whole deterministic world.
//!
//! Nobody ever transmits game state. The one apparent exception,
//! [`Message::Welcome`], carries a *save string* — the same join ticket the
//! single-player game writes — and its body is treated as an opaque blob
//! here; [`crate::sim::Sim::from_save`] is the authority that validates it.

use std::fmt;
use std::fmt::Write as _;

use crate::sim::{CrewFrame, InputFrame, MAX_CREW, PlayerId, Vec2};

/// Magic-plus-version header of every message this build writes. Bump on
/// any breaking change; older versions fail safe as unsupported.
const MAGIC: &str = "SNP2";

/// Why a wire payload was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// Not a Space Trucking payload at all.
    BadMagic,
    /// A Space Trucking payload from a version this build does not read.
    UnsupportedVersion,
    /// Recognised header, malformed body; `line` is 1-based (0 = truncated).
    Parse { line: usize },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a Space Trucking net payload"),
            Self::UnsupportedVersion => write!(f, "payload is from an unsupported version"),
            Self::Parse { line: 0 } => write!(f, "payload ends too early"),
            Self::Parse { line } => write!(f, "malformed payload at line {line}"),
        }
    }
}

impl std::error::Error for WireError {}

/// One protocol message. `Hello`/`Welcome` are the join handshake,
/// `Input`/`Schedule`/`ScheduleRequest` are the lockstep stream, and
/// `Report`/`Progress` are the guild server's idempotent pair.
#[derive(Clone, Debug)]
pub enum Message {
    /// A client announcing itself to the helm as `player`.
    Hello { player: PlayerId },
    /// The helm's join ticket: a complete save plus the schedule index that
    /// snapshot has applied through — the joiner's first tick to apply.
    Welcome { save: String, next_tick: u64 },
    /// One player's input for a future sealed tick.
    Input {
        tick: u64,
        player: PlayerId,
        frame: InputFrame,
    },
    /// One sealed tick's canonical crew inputs, in player order.
    Schedule { tick: u64, frames: CrewFrame },
    /// A replica asking for sealed history it missed, `from_tick` onward.
    ScheduleRequest { from_tick: u64 },
    /// A cluster helm reporting its ship's monotonic delivery tally.
    Report { cluster: u64, deliveries: u32 },
    /// The guild server's merged total. Display state, like the mute flag:
    /// nothing the server says ever enters the deterministic sim.
    Progress { total: u64 },
}

impl Message {
    /// Serialise for the wire. The inverse of [`Message::from_wire`]:
    /// parsing the output reproduces every field bit-exactly.
    #[must_use]
    pub fn to_wire(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail, so the fmt plumbing is dropped.
        match self {
            Self::Hello { player } => {
                let _ = writeln!(out, "{MAGIC} hello {player}");
            }
            Self::Welcome { save, next_tick } => {
                // The save rides as the raw body: reproducing it byte-exact
                // matters more than tokenising it, and `Sim::from_save` is
                // already the defensive parser for its contents.
                let _ = writeln!(out, "{MAGIC} welcome {next_tick}");
                out.push_str(save);
            }
            Self::Input {
                tick,
                player,
                frame,
            } => {
                let _ = write!(out, "{MAGIC} input {tick} {player} ");
                write_frame(&mut out, frame);
            }
            Self::Schedule { tick, frames } => {
                let _ = writeln!(out, "{MAGIC} schedule {tick}");
                for frame in frames {
                    out.push_str("frame ");
                    write_frame(&mut out, frame);
                }
            }
            Self::ScheduleRequest { from_tick } => {
                let _ = writeln!(out, "{MAGIC} request {from_tick}");
            }
            Self::Report {
                cluster,
                deliveries,
            } => {
                let _ = writeln!(out, "{MAGIC} report {cluster} {deliveries}");
            }
            Self::Progress { total } => {
                let _ = writeln!(out, "{MAGIC} progress {total}");
            }
        }
        out
    }

    /// Rebuild a message from [`Message::to_wire`] output.
    pub fn from_wire(s: &str) -> Result<Self, WireError> {
        // The header is split off by hand rather than through `lines()`
        // because a Welcome's body must survive byte-exact, trailing
        // newline (or lack of one) included.
        let (header, body) = s.split_once('\n').unwrap_or((s, ""));
        let mut tokens = header.split_whitespace();
        match tokens.next() {
            Some(MAGIC) => {}
            Some(other) if other.starts_with("SNP") => return Err(WireError::UnsupportedVersion),
            _ => return Err(WireError::BadMagic),
        }
        let at = At(1);
        match tokens.next() {
            Some("hello") => Ok(Self::Hello {
                player: at.player(tokens.next())?,
            }),
            Some("welcome") => Ok(Self::Welcome {
                next_tick: at.token(tokens.next())?,
                save: body.to_owned(),
            }),
            Some("input") => Ok(Self::Input {
                tick: at.token(tokens.next())?,
                player: at.player(tokens.next())?,
                frame: parse_frame(&at, &mut tokens)?,
            }),
            Some("schedule") => {
                let tick = at.token(tokens.next())?;
                let mut frames = [InputFrame::default(); MAX_CREW];
                let mut lines = body.lines();
                for (i, slot) in frames.iter_mut().enumerate() {
                    let at = At(2 + i);
                    let line = lines.next().ok_or(WireError::Parse { line: 0 })?;
                    let mut tokens = line.split_whitespace();
                    if tokens.next() != Some("frame") {
                        return Err(at.err());
                    }
                    *slot = parse_frame(&at, &mut tokens)?;
                }
                Ok(Self::Schedule { tick, frames })
            }
            Some("request") => Ok(Self::ScheduleRequest {
                from_tick: at.token(tokens.next())?,
            }),
            Some("report") => Ok(Self::Report {
                cluster: at.token(tokens.next())?,
                deliveries: at.token(tokens.next())?,
            }),
            Some("progress") => Ok(Self::Progress {
                total: at.token(tokens.next())?,
            }),
            _ => Err(at.err()),
        }
    }
}

/// One frame as ten tokens plus a newline: pointer x/y as f32 bit
/// patterns (exact), the seven buttons/modifiers as 0/1, reseed as `-` or
/// 16 hex digits.
fn write_frame(out: &mut String, frame: &InputFrame) {
    let reseed = frame
        .reseed
        .map_or_else(|| "-".to_owned(), |seed| format!("{seed:016x}"));
    let _ = writeln!(
        out,
        "{:08x} {:08x} {} {} {} {} {} {} {} {reseed}",
        frame.pointer.x.to_bits(),
        frame.pointer.y.to_bits(),
        u8::from(frame.press),
        u8::from(frame.held),
        u8::from(frame.release),
        u8::from(frame.toggle_pause),
        u8::from(frame.toggle_warp),
        u8::from(frame.shift),
        u8::from(frame.night),
    );
}

/// The ten frame tokens back into an [`InputFrame`].
fn parse_frame<'a>(
    at: &At,
    tokens: &mut impl Iterator<Item = &'a str>,
) -> Result<InputFrame, WireError> {
    Ok(InputFrame {
        pointer: Vec2::new(
            f32::from_bits(at.hex32(tokens.next())?),
            f32::from_bits(at.hex32(tokens.next())?),
        ),
        press: at.bit(tokens.next())?,
        held: at.bit(tokens.next())?,
        release: at.bit(tokens.next())?,
        toggle_pause: at.bit(tokens.next())?,
        toggle_warp: at.bit(tokens.next())?,
        shift: at.bit(tokens.next())?,
        night: at.bit(tokens.next())?,
        reseed: at.opt_hex64(tokens.next())?,
    })
}

/// Token-parsing context that remembers which line it is on, so every
/// failure can name its line — the same contract `save.rs`'s Reader keeps.
struct At(usize);

impl At {
    /// A parse error naming this line.
    const fn err(&self) -> WireError {
        WireError::Parse { line: self.0 }
    }

    /// One token parsed to any [`std::str::FromStr`] type.
    fn token<T: std::str::FromStr>(&self, token: Option<&str>) -> Result<T, WireError> {
        token.and_then(|t| t.parse().ok()).ok_or_else(|| self.err())
    }

    /// A bounds-checked crew index.
    fn player(&self, token: Option<&str>) -> Result<PlayerId, WireError> {
        let player: PlayerId = self.token(token)?;
        if usize::from(player) < MAX_CREW {
            Ok(player)
        } else {
            Err(self.err())
        }
    }

    /// One strict boolean token: `0` or `1`, nothing looser.
    fn bit(&self, token: Option<&str>) -> Result<bool, WireError> {
        match token {
            Some("0") => Ok(false),
            Some("1") => Ok(true),
            _ => Err(self.err()),
        }
    }

    /// One token of hex digits, as raw f32 bits.
    fn hex32(&self, token: Option<&str>) -> Result<u32, WireError> {
        token
            .and_then(|t| u32::from_str_radix(t, 16).ok())
            .ok_or_else(|| self.err())
    }

    /// One token that is either `-` or 16 hex digits.
    fn opt_hex64(&self, token: Option<&str>) -> Result<Option<u64>, WireError> {
        match token {
            Some("-") => Ok(None),
            other => other
                .and_then(|t| u64::from_str_radix(t, 16).ok())
                .map(Some)
                .ok_or_else(|| self.err()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::splitmix;

    /// Field-by-field equality, pointer compared as bits. `InputFrame`
    /// deliberately has no `PartialEq` (floats), so the wire tests spell
    /// out exactly which equality they mean.
    fn assert_frames_eq(a: &InputFrame, b: &InputFrame) {
        assert_eq!(a.pointer.x.to_bits(), b.pointer.x.to_bits());
        assert_eq!(a.pointer.y.to_bits(), b.pointer.y.to_bits());
        assert_eq!(a.press, b.press);
        assert_eq!(a.held, b.held);
        assert_eq!(a.release, b.release);
        assert_eq!(a.toggle_pause, b.toggle_pause);
        assert_eq!(a.toggle_warp, b.toggle_warp);
        assert_eq!(a.reseed, b.reseed);
    }

    /// An awkward frame: NaN-payload pointer bits, everything set.
    fn thorny_frame() -> InputFrame {
        InputFrame {
            pointer: Vec2::new(f32::from_bits(0x7FC0_1234), f32::NEG_INFINITY),
            press: true,
            held: true,
            release: true,
            toggle_pause: true,
            toggle_warp: true,
            shift: true,
            night: true,
            reseed: Some(u64::MAX),
        }
    }

    /// One of every message, with edge-case payloads.
    fn menagerie() -> Vec<Message> {
        let mut frames = [InputFrame::default(); MAX_CREW];
        frames[0] = thorny_frame();
        frames[5].pointer = Vec2::new(-0.0, f32::MIN_POSITIVE);
        vec![
            Message::Hello { player: 5 },
            Message::Welcome {
                save: crate::sim::Sim::new(7).save_string(),
                next_tick: u64::MAX,
            },
            Message::Welcome {
                save: "no trailing newline".to_owned(),
                next_tick: 0,
            },
            Message::Input {
                tick: u64::MAX,
                player: 0,
                frame: thorny_frame(),
            },
            Message::Schedule { tick: 42, frames },
            Message::ScheduleRequest { from_tick: 0 },
            Message::Report {
                cluster: u64::MAX,
                deliveries: u32::MAX,
            },
            Message::Progress { total: 123 },
        ]
    }

    #[test]
    fn every_message_round_trips_bit_exactly() {
        for original in menagerie() {
            let wire = original.to_wire();
            let parsed = Message::from_wire(&wire).expect("own wire must parse");
            assert_eq!(wire, parsed.to_wire(), "re-serialisation drifted");
            match (&original, &parsed) {
                (Message::Hello { player: a }, Message::Hello { player: b }) => {
                    assert_eq!(a, b);
                }
                (
                    Message::Welcome {
                        save: sa,
                        next_tick: na,
                    },
                    Message::Welcome {
                        save: sb,
                        next_tick: nb,
                    },
                ) => {
                    assert_eq!(sa, sb, "welcome body must survive byte-exact");
                    assert_eq!(na, nb);
                }
                (
                    Message::Input {
                        tick: ta,
                        player: pa,
                        frame: fa,
                    },
                    Message::Input {
                        tick: tb,
                        player: pb,
                        frame: fb,
                    },
                ) => {
                    assert_eq!(ta, tb);
                    assert_eq!(pa, pb);
                    assert_frames_eq(fa, fb);
                }
                (
                    Message::Schedule {
                        tick: ta,
                        frames: fa,
                    },
                    Message::Schedule {
                        tick: tb,
                        frames: fb,
                    },
                ) => {
                    assert_eq!(ta, tb);
                    for (a, b) in fa.iter().zip(fb) {
                        assert_frames_eq(a, b);
                    }
                }
                (
                    Message::ScheduleRequest { from_tick: a },
                    Message::ScheduleRequest { from_tick: b },
                ) => assert_eq!(a, b),
                (
                    Message::Report {
                        cluster: ca,
                        deliveries: da,
                    },
                    Message::Report {
                        cluster: cb,
                        deliveries: db,
                    },
                ) => {
                    assert_eq!(ca, cb);
                    assert_eq!(da, db);
                }
                (Message::Progress { total: a }, Message::Progress { total: b }) => {
                    assert_eq!(a, b);
                }
                _ => panic!("message changed variant on the wire"),
            }
        }
    }

    #[test]
    fn truncated_or_mangled_wire_fails_safe_never_panics() {
        for original in menagerie() {
            let wire = original.to_wire();
            // Any char-boundary prefix must return a Result, never panic.
            // (Integrity is the transport's job: a mid-token cut of a bare
            // integer can legally parse as a shorter integer, exactly as a
            // save line would. The defensive bar is no panic, and Err for
            // anything structurally short.)
            for (i, _) in wire.char_indices() {
                let _ = Message::from_wire(&wire[..i]);
            }
            // Every header line ends in a required token, so cutting the
            // header at any token boundary must refuse.
            let header = wire.lines().next().unwrap_or("");
            let tokens: Vec<&str> = header.split_whitespace().collect();
            for keep in 0..tokens.len() {
                let truncated = tokens[..keep].join(" ");
                assert!(
                    Message::from_wire(&truncated).is_err(),
                    "header cut to {keep} tokens parsed: {truncated:?}"
                );
            }
            // Header arguments replaced by garbage must refuse.
            for garbage in ["!!!", "NaN", "-1", "99999999999999999999999", "\u{1F680}"] {
                for field in 2..tokens.len() {
                    let mangled: Vec<&str> = tokens
                        .iter()
                        .enumerate()
                        .map(|(i, t)| if i == field { garbage } else { *t })
                        .collect();
                    let mangled = format!(
                        "{}\n{}",
                        mangled.join(" "),
                        wire.split_once('\n').map_or("", |(_, body)| body)
                    );
                    assert!(
                        Message::from_wire(&mangled).is_err(),
                        "field {field} replaced by {garbage:?} parsed anyway"
                    );
                }
            }
        }
        // Wrong or future magic.
        assert!(matches!(Message::from_wire(""), Err(WireError::BadMagic)));
        assert!(matches!(
            Message::from_wire("XXX hello 0"),
            Err(WireError::BadMagic)
        ));
        assert!(matches!(
            Message::from_wire("SNP9 hello 0"),
            Err(WireError::UnsupportedVersion)
        ));
        // Out-of-range crew indices are refused at the wire.
        assert!(Message::from_wire("SNP2 hello 6").is_err());
        assert!(Message::from_wire("SNP2 input 1 6 0 0 0 0 0 0 0 0 0 -").is_err());
        // Loose booleans are refused: the strict wire has no "2" or "true".
        assert!(Message::from_wire("SNP2 input 1 0 0 0 2 0 0 0 0 -").is_err());
        assert!(Message::from_wire("SNP2 input 1 0 0 0 true 0 0 0 0 -").is_err());
    }

    #[test]
    fn schedule_bodies_fail_safe_line_by_line() {
        let wire = Message::Schedule {
            tick: 1,
            frames: [InputFrame::default(); MAX_CREW],
        }
        .to_wire();
        let lines: Vec<&str> = wire.lines().collect();
        assert_eq!(lines.len(), 1 + MAX_CREW);
        // Fewer than six frame lines ends too early.
        for keep in 0..lines.len() {
            let truncated = lines[..keep].join("\n");
            assert!(
                Message::from_wire(&truncated).is_err(),
                "schedule cut to {keep} lines parsed anyway"
            );
        }
        // Any frame line replaced by garbage names its line.
        for target in 1..lines.len() {
            for garbage in [
                "",
                "frame",
                "frame 0 0 0 0 0 0 0 0 0",
                "noise 0 0 0 0 0 0 0 0 0 -",
            ] {
                let mangled: String = lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| if i == target { garbage } else { line })
                    .collect::<Vec<_>>()
                    .join("\n");
                let result = Message::from_wire(&mangled);
                assert!(
                    matches!(result, Err(WireError::Parse { line }) if line == target + 1 || line == 0),
                    "line {target} replaced by {garbage:?} gave {result:?}"
                );
            }
        }
    }

    #[test]
    fn arbitrary_garbage_never_panics() {
        // Deterministic fuzz: random-ish strings over a spicy alphabet.
        let alphabet: Vec<char> = "SNP2 hello\nframe 0-9abcdefx \u{FFFD}\u{1F680}\t"
            .chars()
            .collect();
        for round in 0_u64..300 {
            let len = (splitmix(round, 0) % 120) as usize;
            let s: String = (0..len)
                .map(|i| alphabet[(splitmix(round, 1 + i as u64) as usize) % alphabet.len()])
                .collect();
            // The only contract for garbage is Result, never panic; if it
            // happens to parse, it must at least re-serialise cleanly.
            if let Ok(message) = Message::from_wire(&s) {
                let _ = message.to_wire();
            }
        }
    }
}
