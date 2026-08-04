//! The save slot: browser `localStorage` on the web, quad-storage's data
//! file natively.
//!
//! Binary-crate module with the final signatures; the persistence stage
//! wires it into the frontend loop and vendors quad-storage's js plugin into
//! `web/` (without the plugin, the browser side has nowhere to write).

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
    let saved_at = stamp?.parse().ok()?;
    Some((save?, saved_at))
}

/// Write the save and stamp it with `now` (seconds since the epoch).
pub fn store(save: &str, now: f64) {
    if let Ok(mut storage) = quad_storage::STORAGE.lock() {
        storage.set(SAVE_KEY, save);
        storage.set(SAVED_AT_KEY, &now.to_string());
    }
}
