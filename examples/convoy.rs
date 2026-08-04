//! The "six client apps" in one command: `cargo run --example convoy`.
//!
//! Two crews fly the canonical delivery voyage over hostile links — six
//! clients on ship A, three on ship B, every message delayed, dropped,
//! duplicated, and reordered — while one guild server tallies both. The
//! trace shows each replica's applied tick and a short state hash, so
//! agreement is visible line by line; the run ends with a PASS/FAIL
//! verdict. This is a dev tool: text is fine here, the game renders none.

use space_trucking::net::guild::GuildServer;
use space_trucking::net::harness::{CONVOY_SEED, Convoy, LinkProfile, delivery_voyage};
use space_trucking::net::session::state_hash;
use space_trucking::sim::Sim;

/// One trace line: sealed tick, then `applied:hash` per replica. Agreement
/// reads as identical hashes wherever the applied ticks match.
fn trace(label: &str, convoy: &Convoy, guild: &GuildServer) {
    let replicas: Vec<String> = convoy
        .endpoints
        .iter()
        .map(|ep| {
            ep.client.sim().map_or_else(
                || "-".to_owned(),
                |sim| {
                    let hash = state_hash(&sim.save_string()) & 0xFFFF;
                    format!("{}:{hash:04x}", ep.client.applied())
                },
            )
        })
        .collect();
    println!(
        "[{label}] sealed {:>5} | {} | guild {}",
        convoy.sealed(),
        replicas.join(" "),
        guild.total()
    );
}

/// One cluster's final verdict, printed and returned.
fn verdict(label: &str, convoy: &Convoy, converged: bool) -> bool {
    let helm_save = convoy.sim(0).map_or_else(String::new, Sim::save_string);
    let agree = convoy.endpoints.iter().all(|ep| {
        ep.client
            .sim()
            .is_some_and(|sim| sim.save_string() == helm_save)
    });
    let mut replay = Sim::new(CONVOY_SEED);
    for frames in &convoy.sealed_log {
        replay.crew_tick(frames);
    }
    let replayed = replay.save_string() == helm_save;
    let delivered = convoy.sim(0).map_or(0, Sim::deliveries) == 1;
    println!(
        "[{label}] converged {converged} | replicas agree {agree} | \
         sealed-log replay equals live {replayed} | delivered {delivered}"
    );
    converged && agree && replayed && delivered
}

fn main() {
    let (script_a, end_a) = delivery_voyage(6);
    let (script_b, end_b) = delivery_voyage(3);
    let hostile = LinkProfile::hostile();
    let mut a = Convoy::new(CONVOY_SEED, 0xA11CE, 6, 1, &hostile, script_a);
    let mut b = Convoy::new(CONVOY_SEED, 0xB0B, 3, 2, &hostile, script_b);
    let mut guild = GuildServer::new();

    // The voyages: drags, a launch, a warped cruise, a trade, the Guild's
    // hangar steal — each convoy keeps its own pace, both feed one guild.
    let mut next_trace = 0;
    while a.sealed() < end_a || b.sealed() < end_b {
        if a.sealed() < end_a {
            a.step();
            a.exchange_guild(&mut guild);
        }
        if b.sealed() < end_b {
            b.step();
            b.exchange_guild(&mut guild);
        }
        if a.sealed() >= next_trace {
            trace("A", &a, &guild);
            trace("B", &b, &guild);
            next_trace += 1024;
        }
    }
    // Post-voyage steps keep the resend timers alive until the final
    // delivery reports have certainly landed, then everyone catches up.
    for _ in 0..128 {
        a.step();
        a.exchange_guild(&mut guild);
        b.step();
        b.exchange_guild(&mut guild);
    }
    let ok_a = a.drain(600);
    let ok_b = b.drain(600);
    trace("A", &a, &guild);
    trace("B", &b, &guild);

    let pass = verdict("A", &a, ok_a) & verdict("B", &b, ok_b) & (guild.total() == 2);
    println!("guild total {} (want 2)", guild.total());
    println!("{}", if pass { "PASS" } else { "FAIL" });
    if !pass {
        std::process::exit(1);
    }
}
