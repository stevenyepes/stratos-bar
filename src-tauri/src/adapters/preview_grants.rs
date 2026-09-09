use std::collections::{HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Number of canonicalized paths retained. A single `search_files` call grants at
/// most 50, so this holds roughly the last forty searches' worth of results --
/// comfortably more than a user can cycle through before previewing one.
const DEFAULT_CAPACITY: usize = 2048;

#[derive(Debug, PartialEq, Eq)]
pub enum PreviewAccessError {
    /// The path was never surfaced by the backend, so the webview may not fetch it.
    NotGranted,
    NotFound,
    ReadFailed(String),
    /// The `Range` header was present but unsatisfiable against the file's length.
    RangeNotSatisfiable(u64),
}

/// One served slice of a file, plus what the response needs to describe it.
pub struct PreviewBytes {
    pub bytes: Vec<u8>,
    pub total_len: u64,
    /// `None` for a complete 200 response; `Some((start, end_inclusive))` for a 206.
    pub range: Option<(u64, u64)>,
}

/// The set of file paths the webview is allowed to fetch over the `asset:` scheme.
///
/// Nothing is readable by default. A path becomes fetchable only once the backend
/// has itself surfaced it -- a `search_files` hit, an argument to one of the preview
/// commands, or a thumbnail it generated. This replaces `assetProtocol.scope`'s
/// blanket `$HOME/**` glob: the grant is per-path rather than per-glob, so the
/// renderer can no longer request a file the application never offered it.
///
/// Both granting and checking canonicalize, so a symlink resolves to its target and
/// matches only if that target was itself granted.
pub struct PreviewGrants {
    inner: Mutex<Grants>,
    capacity: usize,
}

struct Grants {
    set: HashSet<PathBuf>,
    /// Insertion order, so the oldest grant is the one evicted at capacity.
    order: VecDeque<PathBuf>,
}

impl PreviewGrants {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Grants {
                set: HashSet::new(),
                order: VecDeque::new(),
            }),
            capacity: capacity.max(1),
        }
    }

    /// Records `path` as fetchable. A path that cannot be canonicalized (typically
    /// because it does not exist) grants nothing rather than erroring -- callers
    /// grant opportunistically alongside work they are doing anyway.
    pub fn grant(&self, path: impl AsRef<Path>) {
        let Ok(canonical) = std::fs::canonicalize(path.as_ref()) else {
            return;
        };

        let mut grants = self.inner.lock().unwrap();
        if !grants.set.insert(canonical.clone()) {
            return;
        }
        grants.order.push_back(canonical);
        while grants.order.len() > self.capacity {
            if let Some(evicted) = grants.order.pop_front() {
                grants.set.remove(&evicted);
            }
        }
    }

    pub fn grant_all<P: AsRef<Path>>(&self, paths: impl IntoIterator<Item = P>) {
        for path in paths {
            self.grant(path);
        }
    }

    pub fn is_granted(&self, path: impl AsRef<Path>) -> bool {
        let Ok(canonical) = std::fs::canonicalize(path.as_ref()) else {
            return false;
        };
        self.is_granted_canonical(&canonical)
    }

    /// Checks a path that the caller has already canonicalized. Callers that also
    /// need the canonical form for a subsequent operation (e.g. opening the file)
    /// should canonicalize once and use this instead of `is_granted`, so the check
    /// and the operation cannot observe different filesystem states.
    pub fn is_granted_canonical(&self, canonical: &Path) -> bool {
        self.inner.lock().unwrap().set.contains(canonical)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().unwrap().set.len()
    }
}

impl Default for PreviewGrants {
    fn default() -> Self {
        Self::new()
    }
}

/// Serves a slice of a granted file. `range_header` is the raw `Range` value, if the
/// webview sent one.
///
/// The grant check runs against the caller's raw string on every call: paths arrive
/// from the renderer over an OS-level URI scheme, so nothing about them may be
/// assumed to have been validated upstream. `requested_path` is canonicalized exactly
/// once, and that single canonical path is what both the grant check and the
/// subsequent open/read operate on -- if a symlink in the path were re-resolved
/// separately for each step, a swap in between could let a granted path serve bytes
/// from a file that was never granted.
pub fn read_preview_range(
    grants: &PreviewGrants,
    requested_path: &str,
    range_header: Option<&str>,
) -> Result<PreviewBytes, PreviewAccessError> {
    let canonical_path =
        std::fs::canonicalize(requested_path).map_err(|_| PreviewAccessError::NotGranted)?;

    if !grants.is_granted_canonical(&canonical_path) {
        return Err(PreviewAccessError::NotGranted);
    }

    let mut file = std::fs::File::open(&canonical_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => PreviewAccessError::NotFound,
        _ => PreviewAccessError::ReadFailed(e.to_string()),
    })?;
    let total_len = file
        .metadata()
        .map_err(|e| PreviewAccessError::ReadFailed(e.to_string()))?
        .len();

    let requested = range_header.and_then(|header| parse_byte_range(header, total_len));
    if range_header.is_some() && requested.is_none() {
        return Err(PreviewAccessError::RangeNotSatisfiable(total_len));
    }

    let (start, end, partial) = match requested {
        Some((start, end)) => (start, end, true),
        None => (0, total_len.saturating_sub(1), false),
    };

    let mut bytes = Vec::new();
    if total_len > 0 {
        file.seek(SeekFrom::Start(start))
            .map_err(|e| PreviewAccessError::ReadFailed(e.to_string()))?;
        let wanted = end - start + 1;
        file.take(wanted)
            .read_to_end(&mut bytes)
            .map_err(|e| PreviewAccessError::ReadFailed(e.to_string()))?;
    }

    Ok(PreviewBytes {
        bytes,
        total_len,
        range: partial.then_some((start, end)),
    })
}

/// Parses a single-range `bytes=` header against a known file length, returning an
/// inclusive `(start, end)`. Multi-range requests and anything unsatisfiable yield
/// `None`; no webview this app targets issues multi-range for media playback.
pub fn parse_byte_range(header: &str, total_len: u64) -> Option<(u64, u64)> {
    let spec = header.trim().strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (raw_start, raw_end) = spec.split_once('-')?;
    let (raw_start, raw_end) = (raw_start.trim(), raw_end.trim());

    if total_len == 0 {
        return None;
    }
    let last = total_len - 1;

    if raw_start.is_empty() {
        // Suffix form: `bytes=-500` means the final 500 bytes.
        let suffix: u64 = raw_end.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        return Some((total_len.saturating_sub(suffix), last));
    }

    let start: u64 = raw_start.parse().ok()?;
    if start > last {
        return None;
    }
    let end = if raw_end.is_empty() {
        last
    } else {
        raw_end.parse::<u64>().ok()?.min(last)
    };
    (start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(path: &Path, contents: &[u8]) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(contents).unwrap();
    }

    #[test]
    fn test_ungranted_path_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("secret.txt");
        write_file(&secret, b"private");

        let grants = PreviewGrants::new();
        let result = read_preview_range(&grants, secret.to_str().unwrap(), None);

        assert_eq!(result.err(), Some(PreviewAccessError::NotGranted));
    }

    #[test]
    fn test_granted_path_is_served() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("photo.png");
        write_file(&file, b"image-bytes");

        let grants = PreviewGrants::new();
        grants.grant(&file);

        let served = read_preview_range(&grants, file.to_str().unwrap(), None).unwrap();
        assert_eq!(served.bytes, b"image-bytes");
        assert_eq!(served.total_len, 11);
        assert!(served.range.is_none());
    }

    #[test]
    fn test_symlink_to_ungranted_target_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let granted = dir.path().join("granted.png");
        let secret = dir.path().join("id_rsa");
        write_file(&granted, b"ok");
        write_file(&secret, b"PRIVATE KEY");

        let grants = PreviewGrants::new();
        grants.grant(&granted);

        // A symlink the renderer plants (or finds) inside a granted directory must
        // not inherit the grant: canonicalization resolves it to the real target.
        let link = dir.path().join("innocent.png");
        std::os::unix::fs::symlink(&secret, &link).unwrap();

        let result = read_preview_range(&grants, link.to_str().unwrap(), None);
        assert_eq!(result.err(), Some(PreviewAccessError::NotGranted));
    }

    #[test]
    fn test_symlink_to_granted_target_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.png");
        write_file(&real, b"pixels");

        let grants = PreviewGrants::new();
        grants.grant(&real);

        let link = dir.path().join("alias.png");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let served = read_preview_range(&grants, link.to_str().unwrap(), None).unwrap();
        assert_eq!(served.bytes, b"pixels");
    }

    #[test]
    fn test_symlink_swap_race_cannot_leak_ungranted_bytes() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let dir = tempfile::tempdir().unwrap();
        let granted = dir.path().join("granted.png");
        let secret = dir.path().join("secret.txt");
        write_file(&granted, b"granted-content");
        write_file(&secret, b"SECRET-DO-NOT-LEAK");

        let grants = Arc::new(PreviewGrants::new());
        grants.grant(&granted);

        let link = dir.path().join("link.png");
        std::os::unix::fs::symlink(&granted, &link).unwrap();

        // Repeatedly repoints `link` between the granted file and the ungranted
        // secret via an atomic rename, racing against the reader below. This
        // reproduces the TOCTOU window the fix closes: a naive implementation
        // that canonicalizes for the grant check but opens the raw, symlinked
        // path could read `secret`'s bytes despite `link` being "granted" at
        // check time.
        let stop = Arc::new(AtomicBool::new(false));
        let counter = Arc::new(AtomicUsize::new(0));
        let swap_dir = dir.path().to_path_buf();
        let swap_granted = granted.clone();
        let swap_secret = secret.clone();
        let swap_link = link.clone();
        let swap_stop = Arc::clone(&stop);
        let swap_counter = Arc::clone(&counter);
        let swapper = thread::spawn(move || {
            let mut toggle = false;
            while !swap_stop.load(Ordering::Relaxed) {
                let target = if toggle { &swap_secret } else { &swap_granted };
                toggle = !toggle;
                let n = swap_counter.fetch_add(1, Ordering::Relaxed);
                let tmp = swap_dir.join(format!("tmp-link-{n}"));
                if std::os::unix::fs::symlink(target, &tmp).is_ok() {
                    let _ = std::fs::rename(&tmp, &swap_link);
                }
            }
        });

        let link_str = link.to_str().unwrap().to_string();
        let grants_reader = Arc::clone(&grants);
        let reader = thread::spawn(move || {
            let mut oks = 0;
            for _ in 0..5000 {
                if let Ok(served) = read_preview_range(&grants_reader, &link_str, None) {
                    oks += 1;
                    // Every successful read must be the granted file's content --
                    // never the secret's, no matter how the symlink was pointed
                    // at the instant of the race.
                    assert_eq!(served.bytes, b"granted-content");
                }
            }
            oks
        });

        let oks = reader.join().unwrap();
        stop.store(true, Ordering::Relaxed);
        swapper.join().unwrap();

        // The race must actually have been exercised, not skipped because the
        // link never resolved to the granted file during the run.
        assert!(oks > 0);
    }

    #[test]
    fn test_grant_evicts_oldest_at_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.png");
        let third = dir.path().join("third.png");
        for path in [&first, &second, &third] {
            write_file(path, b"x");
        }

        let grants = PreviewGrants::with_capacity(2);
        grants.grant(&first);
        grants.grant(&second);
        grants.grant(&third);

        assert_eq!(grants.len(), 2);
        assert!(!grants.is_granted(&first));
        assert!(grants.is_granted(&second));
        assert!(grants.is_granted(&third));
    }

    #[test]
    fn test_grant_of_nonexistent_path_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let grants = PreviewGrants::new();

        grants.grant(dir.path().join("does-not-exist.png"));

        assert_eq!(grants.len(), 0);
    }

    #[test]
    fn test_granted_but_deleted_file_errs_not_panics() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("gone.png");
        write_file(&file, b"bytes");

        let grants = PreviewGrants::new();
        grants.grant(&file);
        let path = file.to_str().unwrap().to_string();
        std::fs::remove_file(&file).unwrap();

        // The grant survives deletion, but canonicalize now fails, so the request
        // is refused rather than panicking on the missing file.
        let result = read_preview_range(&grants, &path, None);
        assert_eq!(result.err(), Some(PreviewAccessError::NotGranted));
    }

    #[test]
    fn test_rangeless_request_serves_the_full_file_even_when_oversized() {
        // Regression test: a rangeless GET (as issued by a plain `<img>` binding,
        // which never sends or retries with a `Range` header) must serve the whole
        // file as a 200, not a synthesized 206 truncated at some fixed cap.
        const OVER_PREVIOUS_CAP: usize = 64 * 1024 * 1024 + 1024;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("huge.png");
        write_file(&file, &vec![0xAB; OVER_PREVIOUS_CAP]);

        let grants = PreviewGrants::new();
        grants.grant(&file);

        let served = read_preview_range(&grants, file.to_str().unwrap(), None).unwrap();
        assert_eq!(served.total_len, OVER_PREVIOUS_CAP as u64);
        assert_eq!(served.bytes.len(), OVER_PREVIOUS_CAP);
        assert_eq!(served.range, None);
    }

    #[test]
    fn test_range_request_serves_the_requested_slice() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mp4");
        write_file(&file, b"0123456789");

        let grants = PreviewGrants::new();
        grants.grant(&file);

        let served =
            read_preview_range(&grants, file.to_str().unwrap(), Some("bytes=2-5")).unwrap();
        assert_eq!(served.bytes, b"2345");
        assert_eq!(served.range, Some((2, 5)));
        assert_eq!(served.total_len, 10);
    }

    #[test]
    fn test_unsatisfiable_range_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mp4");
        write_file(&file, b"0123456789");

        let grants = PreviewGrants::new();
        grants.grant(&file);

        let result = read_preview_range(&grants, file.to_str().unwrap(), Some("bytes=99-"));
        assert_eq!(
            result.err(),
            Some(PreviewAccessError::RangeNotSatisfiable(10))
        );
    }

    #[test]
    fn test_parse_byte_range_forms() {
        assert_eq!(parse_byte_range("bytes=0-", 10), Some((0, 9)));
        assert_eq!(parse_byte_range("bytes=3-4", 10), Some((3, 4)));
        // An end past the last byte clamps rather than failing.
        assert_eq!(parse_byte_range("bytes=3-99", 10), Some((3, 9)));
        assert_eq!(parse_byte_range("bytes=-4", 10), Some((6, 9)));
        assert_eq!(parse_byte_range("bytes=10-", 10), None);
        assert_eq!(parse_byte_range("bytes=5-3", 10), None);
        // Multi-range is declined; no targeted webview sends one for media.
        assert_eq!(parse_byte_range("bytes=0-1,4-5", 10), None);
        assert_eq!(parse_byte_range("items=0-1", 10), None);
        assert_eq!(parse_byte_range("bytes=0-", 0), None);
    }
}
