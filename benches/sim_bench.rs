//! Criterion benchmark

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use game_template::sim::{InputFrame, Sim, TICK_DT, Vec2, WORLD_H, WORLD_W};

const CENTER: Vec2 = Vec2::new(WORLD_W * 0.5, WORLD_H * 0.5);

fn bench_sim_tick(c: &mut Criterion) {
    let input = InputFrame {
        pointer: CENTER,
        attract: true,
        ..InputFrame::default()
    };
    let mut sim = Sim::new(0x5EED);
    // Exactly one tick per iteration: the accumulator lands back on zero
    c.bench_function("sim_tick", |b| {
        b.iter(|| sim.advance(black_box(TICK_DT), black_box(&input)));
    });
}

fn bench_sim_burst(c: &mut Criterion) {
    let input = InputFrame {
        pointer: CENTER,
        burst: true,
        ..InputFrame::default()
    };
    let mut sim = Sim::new(0x5EED);
    c.bench_function("sim_burst", |b| {
        b.iter(|| sim.advance(black_box(TICK_DT), black_box(&input)));
    });
}

criterion_group!(benches, bench_sim_tick, bench_sim_burst);
criterion_main!(benches);
