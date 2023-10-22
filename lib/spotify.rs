use std::collections::HashMap;

use base64::{engine::general_purpose, Engine as _};
use hyper::StatusCode;
use serde::Deserialize;

use crate::{
    errors::{Error, ErrorKind, NPResult},
    twitch::Song,
};

pub const NOW_PLAYING_URL: &str = "https://api.spotify.com/v1/me/player/currently-playing";

pub const CALLBACK_URI: &str = "http://localhost:3000/spotifycallback";

pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

#[derive(Debug, Clone)]
pub struct SpotifyClient {
    http_client: reqwest::Client,
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    //expires_in: Duration,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String,
}

#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyTrack {
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurrentlyPlayingResponse {
    item: Option<SpotifyTrack>,
    is_playing: bool,
}

impl SpotifyTrack {
    fn to_spotify_track_link(&self) -> String {
        format!("https://open.spotify.com/track/{}", &self.id)
    }
}

impl Song for SpotifyTrack {
    fn url(&self) -> String {
        self.to_spotify_track_link()
    }
}

pub fn make_oauth_url(client_id: &str, callback_uri: &str) -> String {
    format!(
        "https://accounts.spotify.com/authorize?client_id={}&redirect_uri={}&scope=user-read-currently-playing&response_type=code",
        client_id, callback_uri
    )
}

fn make_auth_token(client_id: &str, client_secret: &str) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(format!("{}:{}", client_id, client_secret))
}

pub async fn exchange_code(
    code: String,
    client_id: &str,
    client_secret: &str,
) -> NPResult<AuthResponse> {
    let client = reqwest::Client::new();

    let grant_type = String::from("authorization_code");
    let callback_uri = String::from(CALLBACK_URI);

    let mut form_data = HashMap::new();
    form_data.insert("code", &code);
    form_data.insert("grant_type", &grant_type);
    form_data.insert("redirect_uri", &callback_uri);

    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header(
            "Authorization",
            format!("Basic {}", make_auth_token(client_id, client_secret)),
        )
        .form(&form_data)
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => Ok(response.json().await?),
        StatusCode::BAD_REQUEST => Err(Error::new(
            String::from("Invalid authorization code"),
            ErrorKind::SpotifyError,
        )
        .into()),
        _ => Err(Error::new(
            format!("expected status 200, got {}", response.status()),
            ErrorKind::SpotifyError,
        )
        .into()),
    }
}

impl SpotifyClient {
    pub fn new(
        client_id: String,
        client_secret: String,
        access_token: String,
        refresh_token: String,
        //expires_in: Duration,
    ) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            client_id,
            client_secret,
            access_token,
            refresh_token,
            //expires_in,
        }
    }

    pub async fn fetch_current_song(
        &mut self,
    ) -> Result<Option<SpotifyTrack>, Box<dyn std::error::Error>> {
        let resp = self
            .http_client
            .get(NOW_PLAYING_URL)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await?;

        if resp.status() == StatusCode::UNAUTHORIZED {
            self.do_refresh_token().await?;
            return Err("token refresh needed, done".into()); // TODO: make this auto run again
        }

        if resp.status() != StatusCode::OK {
            return Ok(None);
        }

        let track = &resp.json::<CurrentlyPlayingResponse>().await?;

        if !track.is_playing {
            return Ok(None);
        }

        Ok(Some(track.item.clone().ok_or("No track playing")?))
    }

    pub async fn do_refresh_token(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        let mut form_data = HashMap::new();
        form_data.insert("grant_type", "refresh_token");
        form_data.insert("refresh_token", &self.refresh_token);

        let response: RefreshTokenResponse = self
            .http_client
            .post("https://id.twitch.tv/oauth2/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header(
                "Authorization",
                format!(
                    "Basic {}",
                    make_auth_token(self.client_id.as_str(), self.client_secret.as_str())
                ),
            )
            .form(&form_data)
            .send()
            .await?
            .json()
            .await?;

        self.access_token = response.access_token;
        self.refresh_token = response.refresh_token;

        Ok(true)
    }
}
