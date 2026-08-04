//! The save slot: browser `localStorage` on the web, quad-storage's data
//! file natively (`local.data`, gitignored).
//!
//! The wall-clock stamp is stored as `f64::to_bits` in hex — an exact
//! round-trip with no float-formatting drift. Binary-crate module; the
//! renderer stage wires it into the frontend loop.

/// Storage key for the save string.
pub const SAVE_KEY: &str = "space-trucking/save";

/// Storage key for the wall-clock moment the save was written, in seconds
/// since the epoch, so a reload can fast-forward the time away.
pub const SAVED_AT_KEY: &str = "space-trucking/saved_at";

/// Read the save and its timestamp; `None` unless both exist and parse.
pub fn load() -> Option<(String, f64)> {
    // Scoped so the lock drops before the parsing, not after.
    let (save, stamp) = {
        let storage = quad_storage::STORAGE.lock().ok()?;
        (storage.get(SAVE_KEY), storage.get(SAVED_AT_KEY))
    };
    let saved_at = f64::from_bits(u64::from_str_radix(&stamp?, 16).ok()?);
    // The bits always round-trip, but a stamp that is not a finite moment
    // is a corrupt stamp all the same.
    if !saved_at.is_finite() {
        return None;
    }
    Some((save?, saved_at))
}

/// Write the save and stamp it with `now` (seconds since the epoch).
pub fn store(save: &str, now: f64) {
    if let Ok(mut storage) = quad_storage::STORAGE.lock() {
        storage.set(SAVE_KEY, save);
        storage.set(SAVED_AT_KEY, &format!("{:x}", now.to_bits()));
    }
}

/// Read one arbitrary key. The generic little sibling of [`load`], for the
/// slots that are not the save: consent, telemetry, the replay black box.
#[allow(dead_code)] // Consumed as the replay and telemetry slices land.
pub fn get(key: &str) -> Option<String> {
    quad_storage::STORAGE.lock().ok()?.get(key)
}

/// Write one arbitrary key.
pub fn set(key: &str, value: &str) {
    if let Ok(mut storage) = quad_storage::STORAGE.lock() {
        storage.set(key, value);
    }
}

/// Delete one arbitrary key. Removal, not an empty write: a revoked
/// consent must leave nothing behind to misread later.
#[allow(dead_code)] // Consumed as the replay and telemetry slices land.
pub fn remove(key: &str) {
    if let Ok(mut storage) = quad_storage::STORAGE.lock() {
        storage.remove(key);
    }
}
