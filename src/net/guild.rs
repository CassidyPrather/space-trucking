//! The guild server: one global process, one idempotent message pair.
//!
//! Cluster helms report their ship's monotonic delivery tally; the server
//! merges with `max()` per cluster. That single choice makes the whole
//! star topology duplicate-proof, reorder-proof, and restart-proof: a
//! resent or stale `Report` can only repeat or undershoot a counter it
//! already knows, never inflate it. Nothing the server answers enters the
//! deterministic sim — [`super::protocol::Message::Progress`] is display
//! state, like the mute flag.
//!
//! Persistence is a `GLD1` line format in `save.rs`'s voice: versioned
//! magic, defensive parsing that never panics, and a terminating `total`
//! line that doubles as a checksum.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::protocol::{Message, WireError};

/// Magic-plus-version header of every guild save this build writes.
const MAGIC: &str = "GLD1";

/// The central server's whole state: one merged counter per cluster.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildServer {
    /// Cluster id → highest delivery tally ever reported by that cluster.
    counters: BTreeMap<u64, u32>,
}

impl GuildServer {
    /// A server that has heard nothing yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            counters: BTreeMap::new(),
        }
    }

    /// Merge one report and answer with the running total. Idempotent and
    /// commutative: `max()` per cluster, so duplicates and reorderings of
    /// a monotonic counter cannot double-count.
    pub fn on_report(&mut self, cluster: u64, deliveries: u32) -> Message {
        let merged = self.counters.entry(cluster).or_insert(0);
        *merged = (*merged).max(deliveries);
        Message::Progress {
            total: self.total(),
        }
    }

    /// Message-level entry point: reports earn a `Progress`, everything
    /// else is not the server's business and is ignored.
    pub fn on_message(&mut self, msg: &Message) -> Option<Message> {
        match msg {
            Message::Report {
                cluster,
                deliveries,
            } => Some(self.on_report(*cluster, *deliveries)),
            _ => None,
        }
    }

    /// Global progress: the sum of every cluster's merged tally.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.counters.values().map(|&d| u64::from(d)).sum()
    }

    /// Serialise for restart. The inverse of [`GuildServer::from_save`].
    #[must_use]
    pub fn save_string(&self) -> String {
        let mut out = String::new();
        // Writing into a String cannot fail, so the fmt plumbing is dropped.
        let _ = writeln!(out, "{MAGIC}");
        for (cluster, deliveries) in &self.counters {
            let _ = writeln!(out, "cluster {cluster} {deliveries}");
        }
        // Terminator and checksum in one: a truncated save is missing it,
        // and a mangled counter no longer sums to it.
        let _ = writeln!(out, "total {}", self.total());
        out
    }

    /// Rebuild from [`GuildServer::save_string`] output. Anything
    /// malformed fails safe as an error, never a panic — a corrupt file
    /// must not silently zero the galaxy's progress.
    pub fn from_save(s: &str) -> Result<Self, WireError> {
        let mut lines = s.lines().enumerate();
        match lines.next() {
            Some((_, MAGIC)) => {}
            Some((_, other)) if other.starts_with("GLD") => {
                return Err(WireError::UnsupportedVersion);
            }
            _ => return Err(WireError::BadMagic),
        }
        let mut counters = BTreeMap::new();
        for (index, line) in lines {
            let at = WireError::Parse { line: index + 1 };
            let mut tokens = line.split_whitespace();
            match tokens.next() {
                Some("cluster") => {
                    let cluster: u64 = parse_token(tokens.next()).ok_or(at)?;
                    let deliveries: u32 = parse_token(tokens.next()).ok_or(at)?;
                    // A repeated cluster line is a corrupt save, not a merge.
                    if counters.insert(cluster, deliveries).is_some() {
                        return Err(at);
                    }
                }
                Some("total") => {
                    let total: u64 = parse_token(tokens.next()).ok_or(at)?;
                    let server = Self { counters };
                    if server.total() != total {
                        return Err(at);
                    }
                    return Ok(server);
                }
                _ => return Err(at),
            }
        }
        // The terminating total line never arrived.
        Err(WireError::Parse { line: 0 })
    }
}

/// One whitespace token parsed to any [`std::str::FromStr`] type.
fn parse_token<T: std::str::FromStr>(token: Option<&str>) -> Option<T> {
    token.and_then(|t| t.parse().ok())
}

/// The helm-side watcher: turns the sim's monotonic delivery counter
/// into `Report`s.
///
/// Emits only on growth; re-emitting on a timer is the driver's concern,
/// and idempotence upstream makes those resends free.
#[derive(Clone, Debug)]
pub struct ClusterReporter {
    cluster: u64,
    reported: u32,
}

impl ClusterReporter {
    /// A reporter for one cluster that has reported nothing yet.
    #[must_use]
    pub const fn new(cluster: u64) -> Self {
        Self {
            cluster,
            reported: 0,
        }
    }

    /// Watch the tally; a growth earns a fresh `Report`.
    pub const fn watch(&mut self, deliveries: u32) -> Option<Message> {
        if deliveries <= self.reported {
            return None;
        }
        self.reported = deliveries;
        self.last()
    }

    /// The most recent report, for the driver's resend timer. `None`
    /// until there has been anything worth saying.
    #[must_use]
    pub const fn last(&self) -> Option<Message> {
        if self.reported == 0 {
            None
        } else {
            Some(Message::Report {
                cluster: self.cluster,
                deliveries: self.reported,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::harness::FlakyLink;
    use super::*;
    use crate::sim::splitmix;

    #[test]
    fn reports_merge_with_max_and_progress_totals_across_clusters() {
        let mut server = GuildServer::new();
        assert!(matches!(
            server.on_report(7, 2),
            Message::Progress { total: 2 }
        ));
        // A stale duplicate cannot shrink anything.
        assert!(matches!(
            server.on_report(7, 1),
            Message::Progress { total: 2 }
        ));
        assert!(matches!(
            server.on_report(9, 5),
            Message::Progress { total: 7 }
        ));
        assert_eq!(server.total(), 7);
        // Non-reports are not the server's business.
        assert!(
            server
                .on_message(&Message::Progress { total: 99 })
                .is_none()
        );
        assert_eq!(server.total(), 7);
    }

    #[test]
    fn the_guild_server_restarts_without_losing_a_delivery() {
        let mut server = GuildServer::new();
        server.on_report(1, 3);
        server.on_report(2, 5);
        server.on_report(1, 4);
        let saved = server.save_string();

        // Restart, then replay a hostile mix of stale and duplicate
        // reports plus one genuinely new one.
        let mut restarted = GuildServer::from_save(&saved).expect("own save must parse");
        assert_eq!(restarted, server);
        for (cluster, deliveries) in [(1, 1), (2, 5), (1, 4), (1, 2), (2, 3), (1, 5)] {
            restarted.on_report(cluster, deliveries);
        }
        assert_eq!(restarted.total(), 5 + 5, "exactly the true tallies");
        // And the round trip is stable.
        let again = GuildServer::from_save(&restarted.save_string()).expect("re-save must parse");
        assert_eq!(again, restarted);
    }

    #[test]
    fn guild_saves_fail_safe_under_truncation_and_mangling() {
        let mut server = GuildServer::new();
        server.on_report(3, 1);
        server.on_report(0xDEAD_BEEF, 42);
        let save = server.save_string();
        let lines: Vec<&str> = save.lines().collect();
        for keep in 0..lines.len() {
            let truncated = lines[..keep].join("\n");
            assert!(
                GuildServer::from_save(&truncated).is_err(),
                "save truncated to {keep}/{} lines parsed anyway",
                lines.len()
            );
        }
        for target in 0..lines.len() {
            for garbage in [
                "",
                "!!! ???",
                "cluster x y",
                "cluster 3",
                "total NaN",
                "\u{1F680}",
            ] {
                let mangled: String = lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| if i == target { garbage } else { *line })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    GuildServer::from_save(&mangled).is_err(),
                    "line {target} replaced by {garbage:?} parsed anyway"
                );
            }
        }
        // A counter quietly edited no longer matches the checksum.
        let doctored = save.replacen("cluster 3 1", "cluster 3 7", 1);
        assert_ne!(doctored, save);
        assert!(GuildServer::from_save(&doctored).is_err());
        // A repeated cluster line is corruption, not a merge.
        let doubled = save.replacen("cluster 3 1", "cluster 3 1\ncluster 3 1", 1);
        assert!(GuildServer::from_save(&doubled).is_err());
        // Wrong and future magics fail like the sim's saves do.
        assert!(matches!(
            GuildServer::from_save("STV2\n"),
            Err(WireError::BadMagic)
        ));
        assert!(matches!(
            GuildServer::from_save("GLD9\ntotal 0"),
            Err(WireError::UnsupportedVersion)
        ));
        assert!(matches!(
            GuildServer::from_save(""),
            Err(WireError::BadMagic)
        ));
    }

    #[test]
    fn a_reporter_speaks_only_on_growth_and_remembers_its_last_word() {
        let mut reporter = ClusterReporter::new(11);
        assert!(reporter.last().is_none(), "nothing to resend yet");
        assert!(reporter.watch(0).is_none());
        let report = reporter.watch(2);
        assert!(matches!(
            report,
            Some(Message::Report {
                cluster: 11,
                deliveries: 2
            })
        ));
        // Flat or regressing tallies stay quiet; the last word stands.
        assert!(reporter.watch(2).is_none());
        assert!(reporter.watch(1).is_none());
        assert!(matches!(
            reporter.last(),
            Some(Message::Report {
                cluster: 11,
                deliveries: 2
            })
        ));
    }

    /// Property 6 from NETWORKING.md: three clusters' reports through
    /// hostile links — delayed, dropped, duplicated, reordered, and
    /// periodically resent — sum exactly once at the server.
    #[test]
    fn duplicated_and_reordered_reports_count_exactly_once() {
        let clusters: [u64; 3] = [101, 202, 303];
        let mut reporters: Vec<ClusterReporter> =
            clusters.iter().map(|&c| ClusterReporter::new(c)).collect();
        // Nasty links: high duplication, reordering, real drops.
        let mut links: Vec<FlakyLink> = clusters
            .iter()
            .map(|&c| FlakyLink::new(splitmix(0xF1A6, c), 0..12, 50, 300, true))
            .collect();
        let mut server = GuildServer::new();
        let mut tallies = [0_u32; 3];

        for now in 0_u64..600 {
            for (i, reporter) in reporters.iter_mut().enumerate() {
                // A scripted monotonic delivery counter, growing at
                // deterministic pseudo-random moments.
                if splitmix(0x5EED + clusters[i], now) % 37 == 0 {
                    tallies[i] += 1;
                }
                if let Some(report) = reporter.watch(tallies[i]) {
                    links[i].send(now, &report);
                } else if now % 16 == 0 {
                    // The resend timer: free by idempotence, and the only
                    // thing standing between a dropped final report and a
                    // lost delivery.
                    if let Some(report) = reporter.last() {
                        links[i].send(now, &report);
                    }
                }
                for msg in links[i].deliver(now) {
                    server.on_message(&msg);
                }
            }
        }
        // Drain: no new deliveries, resends only, until the queues empty.
        for now in 600_u64..800 {
            for (i, reporter) in reporters.iter_mut().enumerate() {
                if now % 16 == 0 {
                    if let Some(report) = reporter.last() {
                        links[i].send(now, &report);
                    }
                }
                for msg in links[i].deliver(now) {
                    server.on_message(&msg);
                }
            }
        }
        let true_sum: u64 = tallies.iter().map(|&t| u64::from(t)).sum();
        assert!(true_sum > 20, "the scripted counters must actually move");
        assert_eq!(server.total(), true_sum, "exactly-once accounting");
    }
}
