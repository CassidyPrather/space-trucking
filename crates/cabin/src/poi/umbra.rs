//! **The Umbra Market** — floats in Mercury's shadow and only answers
//! hails while the *caller's* clock reads deep night, which should not be
//! possible and is not explained (DESIGN.md). It bottles midnight and
//! sells it. It pays extra for rat-gnawed goods — "aged in transit,
//! artisanal" — and it prices light at **zero**, because light is a rival
//! product: it fences seized lamps and seized portholes cheap, snuffed,
//! in blackout tins.
//!
//! Unfilled: the room still wears the neutral form. Fill in `CHARACTER`
//! and nothing else in this crate — the brief is the module note at
//! [`super`], and `guild.rs` is the worked example. **The Umbra wants
//! darkness**: [`super::Light::burn`] goes to zero or near it, the
//! pendant hangs snuffed, and whatever a customer can see is seen by the
//! ship's own lamps — which is the joke, since the ship's lamps are cargo
//! the market would very much like to buy.

use super::{Character, NEUTRAL};

/// The Umbra Market's own room. Neutral until somebody gives it a face.
pub const CHARACTER: Character = NEUTRAL;
