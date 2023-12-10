pub mod errors;
pub mod soundcloud;
pub mod spotify;
pub mod twitch;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write};

#[derive(Debug, Default, Clone, PartialEq)]
pub enum ServerStatus {
    Running,
    Restarting,
    #[default]
    Stopped,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Config {
    pub soundcloud_enabled: bool,
    pub soundcloud_oauth: Option<String>,

    pub spotify_enabled: bool,
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,

    pub spotify_oauth: Option<String>,
    pub spotify_oauth_refresh: Option<String>,

    pub twitch_client_id: String,
    pub twitch_client_secret: String,
    pub twitch_username: String,

    pub twitch_oauth: Option<String>,
    pub twitch_oauth_refresh: Option<String>,

    pub web_dashboard_enabled: bool,
}

impl Config {
    pub fn from_env(conf: Option<Self>) -> Self {
        // env vars will overwrite anything in json during runtime and save it to json
        match conf {
            Some(mut c) => {
                let mut updated = false;

                // Update fields of c only if the environment variable exists
                if let Ok(value) = std::env::var("SOUNDCLOUD_ENABLED") {
                    c.soundcloud_enabled = parse_string_to_bool(Some(value));
                    updated = true;
                }
                if let Ok(value) = std::env::var("SOUNDCLOUD_OAUTH") {
                    c.soundcloud_oauth = Some(value);
                    updated = true;
                }
                if let Ok(value) = std::env::var("SPOTIFY_ENABLED") {
                    c.spotify_enabled = parse_string_to_bool(Some(value));
                    updated = true;
                }

                if let Ok(value) = std::env::var("SPOTIFY_CLIENT_ID") {
                    c.spotify_client_id = Some(value);
                    updated = true;
                }

                if let Ok(value) = std::env::var("SPOTIFY_CLIENT_SECRET") {
                    c.spotify_client_secret = Some(value);
                    updated = true;
                }

                if let Ok(value) = std::env::var("TWITCH_CLIENT_ID") {
                    c.twitch_client_id = value;
                    updated = true;
                }

                if let Ok(value) = std::env::var("TWITCH_CLIENT_SECRET") {
                    c.twitch_client_secret = value;
                    updated = true;
                }

                if let Ok(value) = std::env::var("TWITCH_USERNAME") {
                    c.twitch_username = value;
                    updated = true;
                }

                if let Ok(value) = std::env::var("WEB_DASHBOARD_ENABLED") {
                    c.web_dashboard_enabled = parse_string_to_bool(Some(value));
                    updated = true;
                }

                if updated {
                    if yes_no_dialog("You have set environment variables differing from the config. Would you like to overwrite the config file?") {
                        if let Err(_) = c.save_to_file() {
                            tracing::error!("Failed to save config.");
                        }
                    }
                }

                c
            }
            None => {
                let spotify_enabled = parse_string_to_bool(std::env::var("SPOTIFY_ENABLED").ok());
                let mut spotify_client_id = None;
                let mut spotify_client_secret = None;
                let mut spotify_oauth = None;
                let mut spotify_oauth_refresh = None;
                if spotify_enabled {
                    spotify_client_id = Some(
                        std::env::var("SPOTIFY_CLIENT_ID")
                            .expect("SPOTIFY_ENABLED is true but SPOTIFY_CLIENT_ID is not set"),
                    );
                    spotify_client_secret = Some(
                        std::env::var("SPOTIFY_CLIENT_SECRET")
                            .expect("SPOTIFY_ENABLED is true but SPOTIFY_CLIENT_SECRET is not set"),
                    );
                    spotify_oauth = std::env::var("SPOTIFY_OAUTH").ok();
                    spotify_oauth_refresh = std::env::var("SPOTIFY_OAUTH_REFRESH").ok();
                }

                let c = Config {
                    soundcloud_enabled: parse_string_to_bool(
                        std::env::var("SOUNDCLOUD_ENABLED").ok(),
                    ),
                    soundcloud_oauth: std::env::var("SOUNDCLOUD_OAUTH").ok(),

                    spotify_enabled,
                    spotify_client_id,
                    spotify_client_secret,

                    spotify_oauth,
                    spotify_oauth_refresh,

                    twitch_client_id: std::env::var("TWITCH_CLIENT_ID")
                        .expect("TWITCH_CLIENT_ID is not set"),
                    twitch_client_secret: std::env::var("TWITCH_CLIENT_SECRET")
                        .expect("TWITCH_CLIENT_SECRET is not set"),
                    twitch_username: std::env::var("TWITCH_USERNAME")
                        .expect("TWITCH_USERNAME is not set"),
                    twitch_oauth: std::env::var("TWITCH_OAUTH").ok(),
                    twitch_oauth_refresh: std::env::var("TWITCH_OAUTH_REFRESH").ok(),
                    web_dashboard_enabled: parse_string_to_bool(
                        std::env::var("WEB_DASHBOARD_ENABLED").ok(),
                    ),
                };

                if yes_no_dialog("Would you like to save the config to a file?") {
                    if let Err(_) = c.save_to_file() {
                        tracing::error!("Failed to save config.");
                    }
                }

                c
            }
        }
    }

    pub fn default_path() -> String {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").expect("HOME is not set");
            format!("{}/.config/nowplaying-ttv/config.json", home)
        }

        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").expect("APPDATA is not set");
            format!("{}/nowplaying-ttv/config.json", appdata)
        }
    }

    pub fn from_json() -> Result<Self, Box<dyn std::error::Error>> {
        let location = std::env::var("CONFIG_FILE").unwrap_or(Self::default_path());
        let mut file = match File::open(&location) {
            Ok(f) => f,
            Err(e) => {
                tracing::error!(
                    "A non fatal error occured while opening {}. {}",
                    location,
                    e
                );
                return Err(
                    format!("A non fatal error occured while opening the file. {}", e).into(),
                );
            }
        };
        let data: Self = match serde_json::from_reader(&mut file) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    "A non fatal error occured while deserializing the file. (The JSON doesn't match) {}",
                    e
                );
                return Err(format!(
                    "A non fatal error occured while deserializing the file. (The JSON doesn't match) {}",
                    e
                )
                .into());
            }
        };

        Ok(data)
    }

    pub fn save_to_file(&self) -> Result<(), Box<dyn std::error::Error>> {
        let location = std::env::var("CONFIG_FILE").unwrap_or(Self::default_path());

        let conf_json = match serde_json::to_string_pretty(&self) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    "A non fatal error occured while serializing the struct into a string. {}",
                    e
                );
                return Err(format!(
                    "A non fatal error occured while serializing the struct into a string. {}",
                    e
                )
                .into());
            }
        };

        match File::create(location.clone()) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(conf_json.as_bytes()) {
                    tracing::error!(
                        "A non fatal error occured while writing the string into the file. {}",
                        e
                    );
                    return Err(format!(
                        "A non fatal error occured while writing the string into the file. {}",
                        e
                    )
                    .into());
                }
            }
            Err(e) => {
                tracing::error!("A non fatal error occured while creating the file. {}", e);
                return Err(
                    format!("A non fatal error occured while creating the file. {}", e).into(),
                );
            }
        };

        tracing::info!("Saved config to {}", location);

        Ok(())
    }
}

fn parse_string_to_bool(s: Option<String>) -> bool {
    match s {
        Some(s) => match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" => true,
            "false" | "0" | "no" | "n" | "off" => false,
            _ => true,
        },
        None => false,
    }
}

fn yes_no_dialog(query: &str) -> bool {
    println!("{} [{}/{}]: ", query, "Y".green(), "n".red());

    let mut input = String::with_capacity(1);

    match std::io::stdin().read_line(&mut input) {
        Ok(_) => {
            let trimmed_input = input.trim().to_lowercase();
            match trimmed_input.as_str() {
                "" => true,
                "\n" => true,
                "y" => true,
                "n" => false,
                _ => false,
            }
        }
        Err(e) => {
            tracing::error!("Unable to read from stdin. Assuming No. ({})", e);
            false
        }
    }
}
