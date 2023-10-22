use serde::Deserialize;

use crate::twitch::Song;

const PLAY_HISTORY_URL: &str = "https://api-v2.soundcloud.com/me/play-history/tracks?limit=1";

#[derive(Debug, Clone)]
pub struct SoundcloudClient {
    http_client: reqwest::Client,
    oauth: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackInfo {
    pub permalink_url: String,
}

impl Song for TrackInfo {
    fn url(&self) -> String {
        self.permalink_url.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CollectionItem {
    track: TrackInfo,
}

#[derive(Debug, Clone, Deserialize)]
struct SoundCloudData {
    collection: Vec<CollectionItem>,
}

impl SoundcloudClient {
    pub fn new(oauth: Option<String>) -> Option<Self> {
        match oauth {
            Some(oauth) => Some(Self {
                http_client: reqwest::Client::new(),
                oauth,
            }),
            None => None,
        }
    }

    pub async fn fetch_current_song(&self) -> Result<TrackInfo, Box<dyn std::error::Error>> {
        let resp = self
            .http_client
            .get(PLAY_HISTORY_URL)
            .header("Authorization", format!("{}", self.oauth))
            .send()
            .await?
            .json::<SoundCloudData>()
            .await?;

        let track_info = &resp
            .collection
            .first()
            .ok_or("No tracks found in play history")?
            .track;

        Ok(track_info.clone())
    }
}
