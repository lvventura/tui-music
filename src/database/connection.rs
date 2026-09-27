use std::{fs, path::Path};

use rusqlite::Connection;

use crate::errors::AppResult;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn load_app_data(&self, app: &mut crate::app::App) -> AppResult<()> {
        app.history = super::repositories::history::list(&self.connection, 100)?;
        app.favorites = super::repositories::favorites::list(&self.connection)?;
        app.playlists = super::repositories::playlists::list(&self.connection)?;
        Ok(())
    }
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        super::migrations::run(&connection)?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}
