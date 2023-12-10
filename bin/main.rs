use std::{process::Stdio, sync::Arc};
use tokio::{
    io,
    sync::{Mutex, Notify},
};

use crate::api::CallbackResponse;
use nowplaying_ttv_lib::{
    errors::{self, Error, ErrorKind, NPResult},
    soundcloud, spotify, twitch, Config, ServerStatus,
};

use clap::Parser;

mod api;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    web_dashboard: bool,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(short, long)]
    internal_port: Option<u16>,
    #[clap(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    {
        ansi_term::enable_ansi_support().unwrap();
    }

    tracing_subscriber::fmt::init();

    tracing::info!("Starting nowplaying-ttv");

    tracing::info!("Loading config");
    let config = match Config::from_json() {
        Ok(c) => Config::from_env(Some(c)),
        Err(_) => Config::from_env(None),
    };

    let args = Args::parse();

    if config.web_dashboard_enabled || args.web_dashboard {
        tracing::info!("Starting web dashboard");
        let exec_path = match get_web_executable_path() {
            Ok(path) => path,
            Err(e) => {
                tracing::error!("Failed to get web executable path: {}", e);
                return;
            }
        };

        let mut cmd = tokio::process::Command::new(exec_path);

        let mut child = Box::pin(
            cmd.stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start web dashboard"),
        );

        let mut stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                tracing::error!("Failed to get stdout of web dashboard");
                return;
            }
        };

        tokio::spawn(async move {
            match child.wait().await {
                Ok(_) => {
                    tracing::info!("Web dashboard exited");
                }
                Err(e) => {
                    tracing::error!("Web dashboard exited with error: {}", e);
                }
            }
        });

        tokio::spawn(async move {
            match io::copy(&mut stdout, &mut io::stdout()).await {
                Ok(_) => {
                    tracing::info!("stdout of web dashboard exited");
                }
                Err(e) => {
                    tracing::error!("stdout of web dashboard failed: {}", e);
                }
            }
        });
    }

    let config = Arc::new(Mutex::new(config));

    let twitch = twitch::TwitchClient::new(config.clone(), None, None);

    tracing::info!("Checking Twitch OAuth");

    let reauth = match twitch.test_token().await {
        Ok(_) => false,
        Err(_) => {
            tracing::info!("Refreshing Twitch OAuth");
            match twitch.refresh_oauth().await {
                Ok(oauth) => {
                    if let Some(oauth) = oauth {
                        config.lock().await.twitch_oauth = Some(oauth.access_token.clone());
                        config.lock().await.twitch_oauth_refresh = Some(oauth.refresh_token.clone());
                    }
                    false
                }
                Err(e) => {
                    tracing::info!(
                        "Failed to refresh Twitch OAuth, please reauthenticate. Error: {}",
                        e
                    );
                    true
                }
            }
        },
    };

    let status = Arc::new((Mutex::new(ServerStatus::Stopped), Notify::new()));

    let callback_completed = Arc::new(Mutex::new(Arc::new(Notify::new())));
    let mut callback_response = None;

    let mut api_instance = None;

    if reauth {
        callback_response = Some(Arc::new(Mutex::new(CallbackResponse::default())));

        api_instance = Some(Arc::new(
            api::Api::new(
                callback_response.clone(),
                callback_completed.clone(),
                config.clone(),
                status.clone(),
                args.internal_port,
            )
            .await,
        ));
    } else {
        tracing::info!("All auth codes received, starting bot.");
        *status.0.lock().await = ServerStatus::Running;
        callback_completed.lock().await.notify_one();
    }

    loop {
        if let Some(api_instance) = api_instance.clone() {
            tokio::spawn(api::hyper_server(api_instance));
        }

        let worker = twitch_listener_worker(
            config.clone(),
            callback_response.clone(),
            callback_completed.clone(),
            status.clone(),
        )
        .await;
        match worker {
            Ok(_) => {
                tracing::info!("Unexpected worker exit. Restarting...");
            }
            Err(e) => match e.kind {
                errors::ErrorKind::Restarting => {
                    callback_completed.lock().await.notify_waiters();
                    *callback_completed.lock().await = Arc::new(Notify::new());
                }
                _ => {
                    tracing::error!("An error occured: {}", e);
                }
            },
        }
    }
}

fn get_web_executable_path() -> NPResult<String> {
    let cargo_file = include_str!("../Cargo.toml");

    let cargo_toml: toml::Value = toml::from_str(cargo_file).map_err(|e| {
        Error::new(
            format!("Failed to parse Cargo.toml: {}", e),
            ErrorKind::FileError,
        )
    })?;

    if let Some(section) = cargo_toml["bin"].as_array() {
        for entry in section {
            if let Some(src_path) = entry["path"].as_str() {
                if src_path == "web/main.rs" {
                    if let Some(name) = entry["name"].as_str() {
                        #[cfg(target_os = "windows")]
                        {
                            return Ok(format!("{}.exe", name));
                        }

                        #[cfg(not(target_os = "windows"))]
                        {
                            return Ok(name.to_string());
                        }
                    }
                }
            }
        }
    }

    Err(Error::new(
        String::from("Failed to find web executable path"),
        ErrorKind::FileError,
    ))
}

async fn twitch_listener_worker(
    config: Arc<Mutex<Config>>,
    callback_response: Option<Arc<Mutex<CallbackResponse>>>,
    callback_completed: Arc<Mutex<Arc<Notify>>>,
    status: Arc<(Mutex<ServerStatus>, Notify)>,
) -> NPResult<()> {
    let cb = callback_completed.lock().await.clone();

    tokio::select! {
        _ = cb.notified() => {},
        _ = status.1.notified() => {
            return Err(Error::new(String::from("Restarting"), errors::ErrorKind::Restarting));
        }
    }

    tracing::info!("Starting Twitch listener");

    if !config.lock().await.soundcloud_enabled && !config.lock().await.spotify_enabled {
        tracing::warn!(
            "Soundcloud and Spotify are both disabled. The application will not work as intended."
        )
    }

    let sc = soundcloud::SoundcloudClient::new(config.lock().await.soundcloud_oauth.clone())
        .map(|client| Arc::new(client));

    let mut spot = None;

    if let Some(cr) = callback_response.clone() {
        if config.lock().await.spotify_enabled {
            if let Some(spotify_auth) = cr.lock().await.spotify_auth.clone() {
                let client_id = config
                    .lock()
                    .await
                    .spotify_client_id
                    .clone()
                    .expect("SPOTIFY_CLIENT_ID is not set");

                let secret = config
                    .lock()
                    .await
                    .spotify_client_secret
                    .clone()
                    .expect("SPOTIFY_CLIENT_SECRET is not set");

                config.lock().await.spotify_oauth = Some(spotify_auth.access_token.clone());
                config.lock().await.spotify_oauth_refresh =
                    Some(spotify_auth.refresh_token.clone());

                spot = Some(Arc::new(Mutex::new(spotify::SpotifyClient::new(
                    client_id,
                    secret,
                    spotify_auth.access_token,
                    spotify_auth.refresh_token,
                ))));
            }
        };

        if let Some(twitch_auth) = cr.lock().await.twitch_auth.clone() {
            config.lock().await.twitch_oauth = Some(twitch_auth.access_token.clone());
            config.lock().await.twitch_oauth_refresh = Some(twitch_auth.refresh_token.clone());
        }
    }

    let mut twitch = twitch::TwitchClient::new(config.clone(), sc, spot);

    if let Err(_) = config.lock().await.save_to_file() {
        tracing::error!("Failed to save config.");
    }

    tokio::select! {
        listener = twitch.listener(status.clone()) => {
            listener?;
        },
        _ = status.1.notified() => {
            return Err(Error::new(String::from("Restarting"), errors::ErrorKind::Restarting));
        }
    }

    Ok(())
}
