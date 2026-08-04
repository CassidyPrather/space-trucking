//! Criterion benchmark over the sim's hot paths: the per-frame tick, the
//! offline catch-up loop, and the dock-arrival visit generation.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use space_trucking::sim::{InputFrame, POIS, ShipState, Sim, TICK_DT, Vec2, layout};

/// One hour of sim time, in ticks.
const HOUR_TICKS: u64 = 216_000;

/// A press-and-hold input frame at a world position.
fn press_at(pos: Vec2) -> InputFrame {
    InputFrame {
        pointer: pos,
        press: true,
        held: true,
        ..InputFrame::default()
    }
}

/// A sim a second into a Guild-to-Venus leg: select the POI, pull the
/// launch lever, cruise briefly.
fn mid_travel() -> Sim {
    let mut sim = Sim::new(0x5EED);
    sim.advance(0.0, &press_at(POIS[0].pos));
    let lever = Vec2::new(
        layout::LAUNCH_LEVER.w.mul_add(0.5, layout::LAUNCH_LEVER.x),
        layout::LAUNCH_LEVER.h.mul_add(0.5, layout::LAUNCH_LEVER.y),
    );
    sim.advance(0.0, &press_at(lever));
    for _ in 0..60 {
        sim.advance(TICK_DT, &InputFrame::default());
    }
    sim
}

fn bench_travel_tick(c: &mut Criterion) {
    let base = mid_travel();
    let input = InputFrame::default();
    c.bench_function("sim_travel_tick", |b| {
        b.iter_batched_ref(
            || base.clone(),
            |sim| sim.advance(black_box(TICK_DT), black_box(&input)),
            BatchSize::SmallInput,
        );
    });
}

fn bench_fast_forward_hour(c: &mut Criterion) {
    let base = mid_travel();
    c.bench_function("sim_fast_forward_hour", |b| {
        b.iter_batched_ref(
            || base.clone(),
            |sim| sim.fast_forward(black_box(HOUR_TICKS)),
            BatchSize::PerIteration,
        );
    });
}

/// A sim one tick short of the Venus dock, so a single advance runs the
/// whole arrival: visit counting, value jitter, and shelf generation.
fn bench_barter_regen(c: &mut Criterion) {
    let mut base = mid_travel();
    let ShipState::Traveling {
        progress,
        leg_ticks,
        ..
    } = base.ship().state
    else {
        unreachable!("mid_travel() travels")
    };
    base.fast_forward(leg_ticks - progress - 1);
    let input = InputFrame::default();
    c.bench_function("sim_barter_regen", |b| {
        b.iter_batched_ref(
            || base.clone(),
            |sim| sim.advance(black_box(TICK_DT), black_box(&input)),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_travel_tick,
    bench_fast_forward_hour,
    bench_barter_regen
);
criterion_main!(benches);
