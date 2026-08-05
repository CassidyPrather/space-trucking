//! Criterion benchmark over the sim's hot paths: the per-frame tick, the
//! offline catch-up loop, the dock-arrival visit generation, and the
//! lockstep crew tick under a full crew.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use space_trucking::sim::{
    CrewFrame, InputFrame, MAX_CREW, POIS, ShipState, Sim, TICK_DT, Vec2, layout,
};

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
    sim.advance(0.0, &press_at(space_trucking::sim::poi_pos(3, 0)));
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

/// A docked sim with as many players as pieces mid-drag: every crew tick
/// re-resolves one drop legality per dragging player, the per-player cost
/// a lockstep replica pays each sealed tick.
fn bench_crew_tick(c: &mut Criterion) {
    let mut base = Sim::new(0x5EED);
    let centers: Vec<Vec2> = base
        .pieces()
        .iter()
        .take(MAX_CREW)
        .map(|piece| {
            let rect = layout::piece_rect(piece);
            Vec2::new(rect.w.mul_add(0.5, rect.x), rect.h.mul_add(0.5, rect.y))
        })
        .collect();
    assert_eq!(
        centers.len(),
        MAX_CREW,
        "seed must offer a piece per player"
    );
    let mut lift: CrewFrame = [InputFrame::default(); MAX_CREW];
    let mut drag: CrewFrame = [InputFrame::default(); MAX_CREW];
    for (player, &pos) in centers.iter().enumerate() {
        lift[player] = press_at(pos);
        drag[player] = InputFrame {
            pointer: pos,
            held: true,
            ..InputFrame::default()
        };
    }
    base.crew_tick(&lift);
    c.bench_function("sim_crew_tick", |b| {
        b.iter_batched_ref(
            || base.clone(),
            |sim| sim.crew_tick(black_box(&drag)),
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_travel_tick,
    bench_fast_forward_hour,
    bench_barter_regen,
    bench_crew_tick
);
criterion_main!(benches);
