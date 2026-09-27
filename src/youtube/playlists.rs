use crate::errors::AppResult;

use super::{client::YoutubeClient, models::Song};

pub async fn songs(_client: &YoutubeClient, _playlist_id: &str) -> AppResult<Vec<Song>> {
    Ok(Vec::new())
}
