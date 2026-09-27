use rusqlite::Connection;

use crate::{errors::AppResult, youtube::models::Song};

pub fn record(
    connection: &Connection,
    song_id: &str,
    duration: u64,
    completed: bool,
) -> AppResult<()> {
    connection.execute(
        "INSERT INTO history (song_id, play_duration, completed) VALUES (?1, ?2, ?3)",
        (song_id, duration as i64, completed as i64),
    )?;
    Ok(())
}

pub fn list(connection: &Connection, limit: u32) -> AppResult<Vec<Song>> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.title, s.artist, s.duration_seconds, s.thumbnail
         FROM history h JOIN songs s ON s.id = h.song_id
         ORDER BY h.played_at DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit], |row| {
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

pub fn remove_song(connection: &Connection, song_id: &str) -> AppResult<()> {
    connection.execute("DELETE FROM history WHERE song_id = ?1", [song_id])?;
    Ok(())
}
