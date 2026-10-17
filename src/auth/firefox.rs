use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::errors::{AppError, AppResult};

use super::cookie::CookieHeader;

/// Vendors whose `cookies.sqlite` is a plain SQLite database (no DPAPI).
fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let base = PathBuf::from(appdata);
            for vendor in ["Mozilla\\Firefox", "Waterfox", "librewolf", "Floorp"] {
                roots.push(base.join(vendor));
            }
        }
    } else if let Some(home) = dirs::home_dir() {
        for vendor in [".mozilla/firefox", ".waterfox", ".librewolf", ".floorp"] {
            roots.push(home.join(vendor));
        }
    }
    roots
}

/// Finds every `cookies.sqlite` under a vendor root, up to a few levels deep.
pub fn find_profiles(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect_profiles(root, 3, &mut found);
    found
}

fn collect_profiles(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let direct = dir.join("cookies.sqlite");
    if direct.is_file() {
        out.push(direct);
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_profiles(&path, depth - 1, out);
        }
    }
}

/// Returns the most recently modified Firefox-family cookie database.
pub fn most_recent_cookie_db() -> Option<PathBuf> {
    let mut all: Vec<PathBuf> = candidate_roots()
        .iter()
        .flat_map(|root| find_profiles(root))
        .collect();
    all.sort_by(|left, right| modified(right).cmp(&modified(left)));
    all.into_iter().next()
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

/// Copies the cookie database (plus WAL/SHM) to a temporary directory, reads
/// the YouTube cookies and always removes the snapshot afterwards.
pub fn read_cookies(cookies_path: &Path) -> AppResult<CookieHeader> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let snapshot_dir = std::env::temp_dir().join(format!("yt-dj-firefox-{nanos}"));
    fs::create_dir_all(&snapshot_dir)?;
    let outcome = read_from_snapshot(cookies_path, &snapshot_dir);
    let _ = fs::remove_dir_all(&snapshot_dir);
    outcome
}

fn read_from_snapshot(cookies_path: &Path, snapshot_dir: &Path) -> AppResult<CookieHeader> {
    let destination = snapshot_dir.join("cookies.sqlite");
    copy_snapshot(cookies_path, &destination)?;
    let pairs = query_cookies(&destination)?;
    CookieHeader::from_pairs(pairs, false)
}

fn copy_snapshot(source: &Path, destination: &Path) -> AppResult<()> {
    fs::copy(source, destination).map_err(|error| {
        AppError::Provider(format!(
            "no se pudo copiar la base de cookies (¿Firefox abierto? ciérralo e inténtalo de nuevo): {error}"
        ))
    })?;
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{}", source.display(), suffix));
        if side.is_file() {
            let target = PathBuf::from(format!("{}{}", destination.display(), suffix));
            let _ = fs::copy(&side, &target);
        }
    }
    Ok(())
}

/// Reads the authentication cookies from a copied Firefox database.
pub fn query_cookies(path: &Path) -> AppResult<Vec<(String, String)>> {
    let connection = rusqlite::Connection::open(path)?;
    let mut statement =
        connection.prepare("SELECT name, value FROM moz_cookies WHERE host LIKE '%youtube.com'")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut pairs = Vec::new();
    for row in rows {
        pairs.push(row?);
    }
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    use super::{find_profiles, query_cookies};

    fn unique_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("yt-dj-{tag}-{nanos}"))
    }

    #[test]
    fn profiles_are_found_below_a_vendor_root() {
        let root = unique_dir("firefox-root");
        let profile = root.join("Profiles").join("abc.default-release");
        fs::create_dir_all(&profile).unwrap();
        fs::write(profile.join("cookies.sqlite"), b"not really sqlite").unwrap();

        let found = find_profiles(&root);
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("abc.default-release\\cookies.sqlite")
            || found[0].ends_with("abc.default-release/cookies.sqlite"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn youtube_cookies_are_read_from_a_firefox_schema() {
        let dir = unique_dir("firefox-db");
        fs::create_dir_all(&dir).unwrap();
        let db = dir.join("cookies.sqlite");
        let connection = rusqlite::Connection::open(&db).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE moz_cookies (host TEXT, name TEXT, value TEXT);
                 INSERT INTO moz_cookies VALUES ('.youtube.com', 'SAPISID', 'secret');
                 INSERT INTO moz_cookies VALUES ('.youtube.com', 'SID', 'sid-value');
                 INSERT INTO moz_cookies VALUES ('.youtube.com', 'noise', 'ignored');
                 INSERT INTO moz_cookies VALUES ('.example.com', 'SAPISID', 'other');",
            )
            .unwrap();
        drop(connection);

        let pairs = query_cookies(&db).unwrap();
        assert!(pairs.iter().any(|(name, value)| name == "SAPISID" && value == "secret"));
        assert!(pairs.iter().any(|(name, _)| name == "SID"));

        let _ = fs::remove_dir_all(&dir);
    }
}
