use reqwest::Client;

#[derive(Clone)]
pub struct YoutubeClient {
    http: Client,
}

impl YoutubeClient {
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }
}
