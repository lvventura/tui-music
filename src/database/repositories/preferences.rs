use rusqlite::Connection;

use crate::errors::AppResult;

pub fn set(connection: &Connection, key: &str, value: &str) -> AppResult<()> {
    connection.execute(
        "INSERT OR REPLACE INTO preferences (key, value) VALUES (?1, ?2)",
        (key, value),
    )?;
    Ok(())
}

pub fn get(connection: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?)
}

use rusqlite::OptionalExtension;
