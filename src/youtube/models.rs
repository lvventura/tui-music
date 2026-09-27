use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
    pub webpage_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub songs: Vec<Song>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SearchResult {
    Song(Song),
    Artist(Artist),
    Album(Album),
    Playlist(Playlist),
}
