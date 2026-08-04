//! Performance budgets, enforced.
//!
//! DESIGN.md asks for profiling metrics beyond the wasm size budget; these
//! are them, documented in `docs/BUDGETS.md`. Each test is `#[ignore]`d so
//! the ordinary debug `cargo test` skips it; CI runs this target in release
//! (`cargo test --release --test perf -- --ignored`), where the ceilings
//! hold ~50-100x headroom over measured numbers — they exist to catch a
//! regression in *kind* (an accidental O(n²), a busy loop), not a few
//! percent of drift. Retune a ceiling deliberately, in `docs/BUDGETS.md`,
//! in the same change that needs it.

use std::time::Instant;

use space_trucking::net::harness::delivery_voyage;
use space_trucking::net::{Convoy, GuildServer, LinkProfile};
use space_trucking::sim::{CrewFrame, InputFrame, MAX_CREW, Sim, Vec2};

/// One wall-clock ceiling, asserted with the measurement in the message.
fn within(label: &str, budget_ms: u128, work: impl FnOnce()) {
    let start = Instant::now();
    work();
    let took = start.elapsed().as_millis();
    assert!(
        took <= budget_ms,
        "{label}: {took} ms over the {budget_ms} ms budget — \
         see docs/BUDGETS.md before retuning"
    );
}

/// Offline catch-up: one simulated hour must stay imperceptible at load.
/// Measured ~2 ms; the whole six-hour cap must land well under a second.
#[test]
#[ignore = "perf budget; CI runs this target in release"]
fn an_hour_of_catch_up_fits_the_budget() {
    let mut sim = Sim::new(0xBEEF);
    within("fast_forward(1h)", 250, || {
        sim.fast_forward(216_000);
    });
}

/// Lockstep's inner loop: a full crew ticking must stay effectively free.
/// Measured ~127 ns/tick; 100k ticks is ~28 minutes of sealed play.
#[test]
#[ignore = "perf budget; CI runs this target in release"]
fn a_hundred_thousand_crew_ticks_fit_the_budget() {
    let mut sim = Sim::new(0xBEEF);
    let mut frames = CrewFrame::default();
    for (i, frame) in frames.iter_mut().enumerate() {
        *frame = InputFrame {
            pointer: Vec2::new(30.0_f32.mul_add(i as f32, 40.0), 460.0),
            held: true,
            ..InputFrame::default()
        };
    }
    within("100k crew_ticks x 6 players", 250, || {
        for _ in 0..100_000 {
            sim.crew_tick(&frames);
        }
    });
}

/// The whole hostile-network convoy voyage — six replicas, flaky links, a
/// delivery — is the heaviest thing the test suite does. Measured ~0.1 s.
#[test]
#[ignore = "perf budget; CI runs this target in release"]
fn the_convoy_voyage_fits_the_budget() {
    let (script, end) = delivery_voyage(MAX_CREW);
    let mut guild = GuildServer::new();
    let mut convoy = Convoy::new(176, 0xF1A7, MAX_CREW, 1, &LinkProfile::hostile(), script);
    within("convoy delivery voyage", 5_000, || {
        while convoy.sealed() < end {
            convoy.step();
            convoy.exchange_guild(&mut guild);
        }
        convoy.drain(10_000);
    });
    assert!(
        convoy.converged(),
        "voyage must still converge inside budget"
    );
}

/// Saves ride every state-changing cue plus a ten-second timer; a thousand
/// round-trips bounds the per-save cost at well under a millisecond.
#[test]
#[ignore = "perf budget; CI runs this target in release"]
fn a_thousand_save_round_trips_fit_the_budget() {
    let mut sim = Sim::new(0xBEEF);
    sim.fast_forward(10_000);
    within("1000 x save_string + from_save", 500, || {
        for _ in 0..1000 {
            let save = sim.save_string();
            sim = Sim::from_save(&save).expect("own save must load");
        }
    });
}
