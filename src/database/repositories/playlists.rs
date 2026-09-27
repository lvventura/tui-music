use rusqlite::Connection;

use crate::{errors::AppResult, youtube::models::Song};

pub fn create(connection: &Connection, id: &str, name: &str) -> AppResult<()> {
    connection.execute(
        "INSERT INTO playlists (id, name) VALUES (?1, ?2)",
        (id, name),
    )?;
    Ok(())
}

pub fn delete(connection: &Connection, id: &str) -> AppResult<()> {
    connection.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id])?;
    connection.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
    Ok(())
}

pub fn list(connection: &Connection) -> AppResult<Vec<(String, String)>> {
    let mut statement = connection.prepare("SELECT id, name FROM playlists ORDER BY name")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn add_song(connection: &Connection, playlist_id: &str, song_id: &str) -> AppResult<()> {
    let position: i64 = connection.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_items WHERE playlist_id = ?1",
        [playlist_id],
        |row| row.get(0),
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO playlist_items (playlist_id, song_id, position) VALUES (?1, ?2, ?3)",
        (playlist_id, song_id, position),
    )?;
    Ok(())
}

pub fn remove_song(connection: &Connection, playlist_id: &str, song_id: &str) -> AppResult<()> {
    connection.execute(
        "DELETE FROM playlist_items WHERE playlist_id = ?1 AND song_id = ?2",
        (playlist_id, song_id),
    )?;
    Ok(())
}

pub fn rename(connection: &Connection, playlist_id: &str, name: &str) -> AppResult<()> {
    connection.execute(
        "UPDATE playlists SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        (name, playlist_id),
    )?;
    Ok(())
}

pub fn move_song(
    connection: &Connection,
    playlist_id: &str,
    song_id: &str,
    position: i64,
) -> AppResult<()> {
    connection.execute(
        "UPDATE playlist_items SET position = ?1 WHERE playlist_id = ?2 AND song_id = ?3",
        (position, playlist_id, song_id),
    )?;
    Ok(())
}

pub fn songs(connection: &Connection, playlist_id: &str) -> AppResult<Vec<Song>> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.title, s.artist, s.duration_seconds, s.thumbnail
         FROM playlist_items i JOIN songs s ON s.id = i.song_id
         WHERE i.playlist_id = ?1 ORDER BY i.position",
    )?;
    let rows = statement.query_map([playlist_id], |row| {
        Ok(Song {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            duration: row.get::<_, Option<i64>>(3)?.map(|value| value as u64),
            thumbnail: row.get(4)?,
            webpage_url: None,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
