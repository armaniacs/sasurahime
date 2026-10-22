use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Persistent scan cache: per-directory mtime key so unchanged directories
/// skip the expensive walk. Invalidation is conservative: any mtime mismatch,
/// age expiry, filesystem error or corrupt cache file falls back to a full
/// walk. Deletion paths never consult the cache.
pub const CACHE_FILE_NAME: &str = "scan-cache.json";

/// Cached entries older than this are ignored.
const MAX_AGE_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CacheEntry {
    /// Apparent size (blocks * 512) measured at detect time.
    pub size_bytes: u64,
    /// mtime of the directory itself, in nanoseconds since epoch.
    pub dir_mtime_nanos: u64,
    /// Seconds since epoch when the entry was measured.
    pub scanned_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ScanCache {
    #[serde(default)]
    entries: HashMap<String, CacheEntry>,
}

impl ScanCache {
    /// Loads the cache file. A corrupt or unreadable file falls back to an
    /// empty cache instead of erroring.
    pub(crate) fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Best-effort save: failures are silent (the cache is an optimization).
    pub(crate) fn save(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Cached size for `dir`, or `None` on mtime mismatch, expiry or error.
    pub(crate) fn cached_size(&self, dir: &Path) -> Option<u64> {
        let entry = self.entries.get(dir.to_string_lossy().as_ref())?;
        let now = now_secs();
        if now.saturating_sub(entry.scanned_at) > MAX_AGE_SECS {
            return None;
        }
        let mtime = dir_mtime_nanos(dir)?;
        if mtime != entry.dir_mtime_nanos {
            return None;
        }
        Some(entry.size_bytes)
    }

    pub(crate) fn record(&mut self, dir: &Path, size_bytes: u64) {
        let Some(mtime) = dir_mtime_nanos(dir) else {
            return;
        };
        self.entries.insert(
            dir.to_string_lossy().to_string(),
            CacheEntry {
                size_bytes,
                dir_mtime_nanos: mtime,
                scanned_at: now_secs(),
            },
        );
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn dir_mtime_nanos(dir: &Path) -> Option<u64> {
    std::fs::metadata(dir)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos() as u64)
}

static SCAN_CACHE: OnceLock<Mutex<Option<ScanCache>>> = OnceLock::new();

fn cache_cell() -> &'static Mutex<Option<ScanCache>> {
    SCAN_CACHE.get_or_init(|| Mutex::new(None))
}

/// Enables the cache for this process, loading it from `path`. Cached size
/// lookups are no-ops until this is called.
pub fn init(cache_path: &Path) {
    let cache = ScanCache::load(cache_path);
    let mut cell = cache_cell().lock().unwrap_or_else(|p| p.into_inner());
    *cell = Some(cache);
}

/// Saves the cache back to `path` and keeps it enabled.
pub fn flush(cache_path: &Path) {
    let cell = cache_cell().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(cache) = cell.as_ref() {
        cache.save(cache_path);
    }
}

/// Apparent size of `dir`, using the persistent cache when the directory
/// mtime is unchanged. Falls back to a full walk on any mismatch. Safe under
/// rayon's parallel scan: the cache is mutex-protected.
pub fn cached_dir_size(dir: &Path) -> u64 {
    let mut cell = cache_cell().lock().unwrap_or_else(|p| p.into_inner());
    let Some(cache) = cell.as_mut() else {
        return crate::format::dir_size(dir);
    };
    if let Some(cached) = cache.cached_size(dir) {
        return cached;
    }
    let size = crate::format::dir_size(dir);
    cache.record(dir, size);
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, name: &str, bytes: usize) {
        std::fs::write(dir.join(name), vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn record_and_reuse_unchanged_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_file(tmp.path(), "a", 1024);
        let measured = crate::format::dir_size(tmp.path());
        let mut cache = ScanCache::default();
        assert_eq!(cache.cached_size(tmp.path()), None, "cold cache");
        cache.record(tmp.path(), measured);
        // Reuse without re-walking: the cached value is returned as-is.
        assert_eq!(cache.cached_size(tmp.path()), Some(measured));
    }

    #[test]
    fn changed_directory_mtime_invalidates() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_file(tmp.path(), "a", 1024);
        let mut cache = ScanCache::default();
        cache.record(tmp.path(), 1024);
        // A new direct child changes the directory mtime.
        write_file(tmp.path(), "b", 2048);
        assert_eq!(
            cache.cached_size(tmp.path()),
            None,
            "changed dirs must be re-measured"
        );
    }

    #[test]
    fn expired_entries_are_ignored() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_file(tmp.path(), "a", 1024);
        let mut cache = ScanCache::default();
        cache.record(tmp.path(), 1024);
        let key = tmp.path().to_string_lossy().to_string();
        if let Some(entry) = cache.entries.get_mut(&key) {
            entry.scanned_at = now_secs().saturating_sub(MAX_AGE_SECS + 1);
        }
        assert_eq!(cache.cached_size(tmp.path()), None, "expired entries miss");
    }

    #[test]
    fn corrupt_cache_file_falls_back_to_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("scan-cache.json");
        std::fs::write(&path, "not valid json :::").unwrap();
        let cache = ScanCache::load(&path);
        assert_eq!(cache.entries.len(), 0, "corrupt file loads empty");
        // And a lookup on the empty cache is a miss, not an error.
        assert_eq!(cache.cached_size(tmp.path()), None);
    }

    #[test]
    fn missing_cache_file_loads_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = ScanCache::load(&tmp.path().join("nonexistent.json"));
        assert_eq!(cache.entries.len(), 0);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let tmp = tempfile::TempDir::new().unwrap();
        // The cache file must live outside the measured directory so saving
        // does not change the measured dir's mtime.
        let path = tmp.path().join("other/nested/scan-cache.json");
        let measured_dir = tmp.path().join("data");
        std::fs::create_dir_all(&measured_dir).unwrap();
        write_file(&measured_dir, "a", 512);
        let mut cache = ScanCache::default();
        cache.record(&measured_dir, 512);
        cache.save(&path);
        let loaded = ScanCache::load(&path);
        assert_eq!(loaded.cached_size(&measured_dir), Some(512));
    }

    #[test]
    fn cached_dir_size_matches_dir_size_and_caches() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_file(tmp.path(), "a", 300);
        let _ = SCAN_CACHE.set(Mutex::new(None));
        init(&tmp.path().join("cache.json"));
        let measured = crate::format::dir_size(tmp.path());
        assert_eq!(cached_dir_size(tmp.path()), measured);
        // Second call comes from the cache (same value without re-walking).
        assert_eq!(cached_dir_size(tmp.path()), measured);
        // A direct-child change invalidates and re-measures.
        write_file(tmp.path(), "b", 100);
        let updated = crate::format::dir_size(tmp.path());
        assert_eq!(cached_dir_size(tmp.path()), updated);
    }
}
