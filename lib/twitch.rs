use std::collections::HashMap;
use std::sync::Arc;

use hyper::StatusCode;
use serde::Deserialize;
use tokio::sync::{Mutex, Notify};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::{PrivmsgMessage, ServerMessage};
use twitch_irc::TwitchIRCClient;
use twitch_irc::{ClientConfig, SecureTCPTransport};

use crate::errors::{Error, ErrorKind, NPResult};
use crate::{soundcloud, spotify, Config, ServerStatus};

pub const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
pub const CALLBACK_URI: &str = "http://localhost:3000/callback";

#[derive(Debug, Clone)]
pub struct TwitchClient {
    config: Arc<Mutex<Config>>,
    client: Option<Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>>,
    soundcloud: Option<Arc<soundcloud::SoundcloudClient>>,
    spotify: Option<Arc<Mutex<spotify::SpotifyClient>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub scope: Vec<String>,
    pub token_type: String,
}

pub trait Song: Send {
    fn url(&self) -> String;
}

pub fn make_oauth_url(client_id: &str, callback_uri: &str) -> String {
    format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=chat:read%20chat:edit",
        client_id, callback_uri
    )
}

pub async fn exchange_code(config: Arc<Mutex<Config>>, code: String) -> NPResult<AuthResponse> {
    let client = reqwest::Client::new();

    let grant_type = String::from("authorization_code");
    let callback_uri = String::from(CALLBACK_URI);

    let conf = config.lock().await;

    let mut form_data = HashMap::new();
    form_data.insert("client_id", &conf.twitch_client_id);
    form_data.insert("client_secret", &conf.twitch_client_secret);
    form_data.insert("code", &code);
    form_data.insert("grant_type", &grant_type);
    form_data.insert("redirect_uri", &callback_uri);

    // Invalid authorization code

    let response = client
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form_data)
        .send()
        .await?;

    match response.status() {
        StatusCode::OK => Ok(response.json().await?),
        StatusCode::BAD_REQUEST => Err(Error::new(
            String::from("Invalid authorization code"),
            ErrorKind::TwitchError,
        )
        .into()),
        _ => Err(Error::new(
            format!("expected status 200, got {}", response.status()),
            ErrorKind::HttpError,
        )
        .into()),
    }
}

impl TwitchClient {
    pub fn new(
        config: Arc<Mutex<Config>>,
        soundcloud: Option<Arc<soundcloud::SoundcloudClient>>,
        spotify: Option<Arc<Mutex<spotify::SpotifyClient>>>,
    ) -> Self {
        Self {
            config,
            client: None,
            soundcloud,
            spotify,
        }
    }

    pub async fn refresh_oauth(&self) -> NPResult<Option<AuthResponse>> {
        if self.test_token().await.is_ok() {
            return Ok(None);
        }

        let config = self.config.lock().await;

        let client_id = &config.twitch_client_id;
        let client_secret = &config.twitch_client_secret;
        let grant_type = String::from("refresh_token");
        let refresh_token = match config.twitch_oauth_refresh.clone() {
            Some(token) => token,
            None => {
                return Err(Error::new(
                    String::from("No refresh token"),
                    ErrorKind::TwitchError,
                )
                .into())
            }
        };

        let client = reqwest::Client::new();

        let mut form_data = HashMap::new();
        form_data.insert("client_id", client_id);
        form_data.insert("client_secret", client_secret);
        form_data.insert("grant_type", &grant_type);
        form_data.insert("refresh_token", &refresh_token);

        let response = client
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&form_data)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let auth_response = response.json::<AuthResponse>().await?;

                tracing::info!("Refreshed Twitch OAuth");

                Ok(Some(auth_response))
            }
            _ => Err(Error::new(
                format!("expected status 200, got {}", response.status()),
                ErrorKind::HttpError,
            )
            .into()),
        }
    }

    pub async fn test_token(&self) -> NPResult<()> {
        let config = self.config.lock().await;

        let client = reqwest::Client::new();

        let response = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header(
                "Authorization",
                format!("OAuth {}", match config.twitch_oauth.clone() {
                    Some(oauth) => oauth,
                    None => return Err(Error::new(String::from("No OAuth token"), ErrorKind::TwitchError).into()),
                }),
            )
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(()),
            _ => Err(Error::new(
                format!("expected status 200, got {}", response.status()),
                ErrorKind::HttpError,
            )
            .into()),
        }
    }

    pub async fn listener(&mut self, status: Arc<(Mutex<ServerStatus>, Notify)>) -> NPResult<()> {
        tracing::info!("Connecting to Twitch");

        let conf = self.config.lock().await.clone();

        let credentials = StaticLoginCredentials::new(
            conf.twitch_username.clone(),
            conf.twitch_oauth.clone(),
        );

        let config = ClientConfig::new_simple(credentials);

        let (mut incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

        client
            .join(conf.twitch_username.clone())
            .expect("Failed to join channel");


        self.client = Some(Arc::new(client));

        tracing::info!(
            "Connected to Twitch IRC, joined channel {}",
            conf.twitch_username.clone()
        );

        let self_arc = Arc::new(self.clone());

        tracing::info!("Listening for messages");
        while let Some(message) = incoming_messages.recv().await {
            let self_ref = self_arc.clone();

            match *status.0.lock().await {
                ServerStatus::Stopped => {
                    tracing::info!("Stopping Twitch listener");
                    break;
                }
                ServerStatus::Restarting => {
                    return Err(Error::new(
                        String::from("Restarting"),
                        ErrorKind::Restarting,
                    ));
                }
                _ => {}
            }

            self_ref.handler(message).await;
        }

        Ok(())
    }

    pub async fn handler(&self, msg: ServerMessage) {
        match msg {
            ServerMessage::Privmsg(msg) => self.message_handler(msg).await,
            _ => {}
        }
    }

    pub async fn message_handler(&self, msg: PrivmsgMessage) {
        match msg.message_text.as_str() {
            "!np" => self.now_playing(msg).await,
            "!song" => self.now_playing(msg).await,
            _ => {}
        }
    }

    pub async fn now_playing(&self, origin: PrivmsgMessage) {
        let mut song: Option<Box<dyn Song>> = None;

        if let Some(spotify) = &self.spotify {
            match spotify.lock().await.fetch_current_song().await {
                Ok(track) => {
                    if let Some(track) = track {
                        song = Some(Box::new(track) as Box<dyn Song>);
                    }
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    song = None;
                }
            }
        };

        if song.is_none() {
            if let Some(sc) = &self.soundcloud {
                match sc.fetch_current_song().await {
                    Ok(track) => {
                        song = Some(Box::new(track) as Box<dyn Song>);
                    }
                    Err(e) => {
                        tracing::error!("{}", e);
                        song = None;
                    }
                }
            }
        };

        match song {
            Some(song) => {
                let message = format!("Now playing: {}", song.url());

                if let Some(client) = self.client.as_ref() {
                    if let Err(e) = client.say_in_reply_to(&origin, message).await {
                        tracing::error!("Failed to send message: {:?}", e);
                    }
                }
            }
            None => {
                tracing::info!("No song found playing.");
                return;
            }
        };
    }
}
