#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::{database::migrations, youtube::models::Song};

    fn song(id: &str) -> Song {
        Song {
            id: id.into(),
            title: "Title".into(),
            artist: "Artist".into(),
            duration: Some(120),
            thumbnail: None,
            webpage_url: None,
        }
    }

    #[test]
    fn repositories_persist_library_data() {
        let connection = Connection::open_in_memory().unwrap();
        migrations::run(&connection).unwrap();
        let item = song("song-1");
        super::super::songs::save(&connection, &item).unwrap();
        assert_eq!(
            super::super::songs::get(&connection, "song-1")
                .unwrap()
                .unwrap()
                .id,
            "song-1"
        );
        super::super::history::record(&connection, "song-1", 12, false).unwrap();
        assert_eq!(
            super::super::history::list(&connection, 10).unwrap().len(),
            1
        );
        super::super::favorites::add(&connection, "song-1").unwrap();
        assert!(super::super::favorites::is_favorite(&connection, "song-1").unwrap());
        super::super::favorites::remove(&connection, "song-1").unwrap();
        assert!(!super::super::favorites::is_favorite(&connection, "song-1").unwrap());
        super::super::playlists::create(&connection, "p1", "Playlist").unwrap();
        super::super::playlists::add_song(&connection, "p1", "song-1").unwrap();
        assert_eq!(
            super::super::playlists::songs(&connection, "p1")
                .unwrap()
                .len(),
            1
        );
        super::super::playlists::rename(&connection, "p1", "Renamed").unwrap();
        super::super::playlists::move_song(&connection, "p1", "song-1", 0).unwrap();
        super::super::playlists::remove_song(&connection, "p1", "song-1").unwrap();
    }
}
