use rusqlite::Connection;

use crate::{errors::AppResult, youtube::models::Song};

pub fn save(connection: &Connection, song: &Song) -> AppResult<()> {
    connection.execute(
        "INSERT OR REPLACE INTO songs (id, title, artist, duration_seconds, thumbnail)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            &song.id,
            &song.title,
            &song.artist,
            song.duration.map(|value| value as i64),
            &song.thumbnail,
        ),
    )?;
    Ok(())
}

pub fn get(connection: &Connection, id: &str) -> AppResult<Option<Song>> {
    let mut statement = connection.prepare(
        "SELECT id, title, artist, duration_seconds, thumbnail FROM songs WHERE id = ?1",
    )?;
    let mut rows = statement.query([id])?;
    rows.next()?
        .map(|row| {
            Ok::<Song, rusqlite::Error>(Song {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                duration: row.get::<_, Option<i64>>(3)?.map(|value| value as u64),
                thumbnail: row.get(4)?,
                webpage_url: None,
            })
        })
        .transpose()
        .map_err(Into::into)
}
