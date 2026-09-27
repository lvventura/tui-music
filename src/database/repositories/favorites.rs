use rusqlite::Connection;

use crate::{errors::AppResult, youtube::models::Song};

pub fn add(connection: &Connection, song_id: &str) -> AppResult<()> {
    connection.execute(
        "INSERT OR IGNORE INTO favorites (song_id) VALUES (?1)",
        [song_id],
    )?;
    Ok(())
}

pub fn remove(connection: &Connection, song_id: &str) -> AppResult<()> {
    connection.execute("DELETE FROM favorites WHERE song_id = ?1", [song_id])?;
    Ok(())
}

pub fn is_favorite(connection: &Connection, song_id: &str) -> AppResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM favorites WHERE song_id = ?1)",
        [song_id],
        |row| row.get(0),
    )?)
}

pub fn list(connection: &Connection) -> AppResult<Vec<Song>> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.title, s.artist, s.duration_seconds, s.thumbnail
         FROM favorites f JOIN songs s ON s.id = f.song_id ORDER BY f.added_at DESC",
    )?;
    let rows = statement.query_map([], |row| {
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
