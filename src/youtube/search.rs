use crate::errors::AppResult;

use super::{client::YoutubeClient, models::Song};

pub async fn search(_client: &YoutubeClient, _query: &str) -> AppResult<Vec<Song>> {
    Ok(Vec::new())
}
