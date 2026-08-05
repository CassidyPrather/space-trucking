//! Deterministic lockstep networking: nobody ever transmits game state.
//!
//! The architecture is `docs/NETWORKING.md` made code. Every crew member
//! runs an identical [`crate::sim::Sim`] replica; only inputs travel. The
//! helm is authoritative over *input ordering only*: it seals each tick's
//! canonical [`crate::sim::CrewFrame`] and broadcasts it, replicas apply
//! sealed ticks strictly in order, and a gap means stall, never diverge.
//! The guild server sits above the clusters with one idempotent message
//! pair, so duplicated and reordered reports can never double-count.
//!
//! Everything in here is pure and macroquad-free, like `sim`: no sockets,
//! no clocks, no threads. [`protocol`] is the wire format, [`session`] the
//! Helm/Client state machines, [`guild`] the central server, and
//! [`harness`] the seeded hostile network that CI drives them through —
//! real transports are a later adapter, and the harness is the proof they
//! will be plumbing, not architecture.

pub mod guild;
pub mod harness;
pub mod protocol;
pub mod session;

pub use guild::{ClusterReporter, GuildServer};
pub use harness::{Convoy, FlakyLink, LinkProfile, Script};
pub use protocol::{Message, WireError};
pub use session::{Client, Helm, INPUT_DELAY, Outbound, state_hash};
