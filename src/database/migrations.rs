use rusqlite::Connection;

use crate::errors::AppResult;

pub fn run(connection: &Connection) -> AppResult<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS songs (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            duration_seconds INTEGER,
            thumbnail TEXT
        );
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            song_id TEXT NOT NULL,
            played_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            play_duration INTEGER NOT NULL DEFAULT 0,
            completed INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS favorites (
            song_id TEXT PRIMARY KEY,
            added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS playlists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS playlist_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id TEXT NOT NULL,
            song_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (playlist_id, song_id),
            FOREIGN KEY (playlist_id) REFERENCES playlists(id),
            FOREIGN KEY (song_id) REFERENCES songs(id)
        );",
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO schema_migrations (version) VALUES (1)",
        [],
    )?;
    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version < 2 {
        connection.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_history_played_at ON history (played_at DESC);
             CREATE INDEX IF NOT EXISTS idx_playlist_items_playlist_position
                 ON playlist_items (playlist_id, position);",
        )?;
        connection.execute("INSERT INTO schema_migrations (version) VALUES (2)", [])?;
    }
    Ok(())
}
