use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use nowplaying_ttv_lib::errors::ErrorKind;
use nowplaying_ttv_lib::{errors, spotify, twitch, Config, ServerStatus};
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use sysinfo::{RefreshKind, SystemExt};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Default)]
pub struct CallbackResponse {
    pub twitch_auth: Option<twitch::AuthResponse>,
    pub spotify_auth: Option<spotify::AuthResponse>,
    pub delivered: bool,
    pub ack: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub cpu_usage: f32,
    pub memory_usage: u64, // given in bytes
    pub memory_total: u64, // given in bytes
}

type NPResult<T> = std::result::Result<T, errors::Error>;

pub struct Api {
    pub callback_response: Arc<Mutex<CallbackResponse>>,
    pub callback_completed: Arc<Mutex<Arc<Notify>>>,
    pub config: Arc<Mutex<Config>>,
    pub status: Arc<(Mutex<ServerStatus>, Notify)>,
    pub system_status: Arc<Mutex<Status>>,
}

pub async fn hyper_server(api: Arc<Api>) -> NPResult<()> {
    loop {
        // TODO: clean this, this is gross
        if *api.status.0.lock().await == ServerStatus::Stopped {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    *api.status.0.lock().await = ServerStatus::Running;

    *api.callback_response.lock().await = CallbackResponse::default();

    let api_ref = api.clone();
    let make_svc = make_service_fn(move |_| {
        let api_ref = api_ref.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let api_ref = api_ref.clone();
                async move { api_ref.handle_request(req).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    tracing::info!("REST API running on http://{}", addr);

    let twitch_oauth_url = twitch::make_oauth_url(
        &api.config.lock().await.twitch_client_id,
        twitch::CALLBACK_URI,
    );

    println!(
        "Please visit this URL to authenticate with Twitch: {}",
        twitch_oauth_url
    );

    if api.config.lock().await.spotify_enabled {
        let spotify_oauth_url = spotify::make_oauth_url(
            &api.config
                .lock()
                .await
                .spotify_client_id
                .clone()
                .expect("SPOTIFY_CLIENT_ID is not set"),
            spotify::CALLBACK_URI,
        );
        println!(
            "Please visit this URL to authenticate with Spotify: {}",
            spotify_oauth_url
        );
    }

    tokio::select! {
        server = server => {
            if let Err(e) = server {
                tracing::error!("The server has quit unexpectedly: {}", e);
                *api.status.0.lock().await = ServerStatus::Stopped;
            }
        }
        _ = api.status.1.notified() => {
            tracing::info!("Stopping REST API server");
            *api.status.0.lock().await = ServerStatus::Stopped;
        }
    }

    Ok(())
}

impl Api {
    pub async fn new(
        callback_response: Arc<Mutex<CallbackResponse>>,
        callback_completed: Arc<Mutex<Arc<Notify>>>,
        config: Arc<Mutex<Config>>,
        status: Arc<(Mutex<ServerStatus>, Notify)>,
    ) -> Self {
        let mut system = sysinfo::System::new_with_specifics(RefreshKind::new().with_memory());
        system.refresh_memory();

        let system_status = Arc::new(Mutex::new(Status::default()));
        system_status.lock().await.memory_total = system.total_memory();

        let system_status_ref = system_status.clone();
        tokio::spawn(async move {
            let mut meter = self_meter::Meter::new(std::time::Duration::from_secs(10)).unwrap();
            meter.track_current_thread("main");
            loop {
                let res = meter.scan();
                if let Err(err) = res {
                    tracing::error!("Error scanning meter: {}", err);
                }

                match meter.report() {
                    Some(report) => {
                        let mut system_status = system_status_ref.lock().await;
                        system_status.cpu_usage = report.process_cpu_usage;
                        system_status.memory_usage = report.memory_rss;
                    }
                    None => {}
                }

                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });

        Self {
            callback_response,
            callback_completed,
            config,
            status,
            system_status,
        }
    }

    async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        tracing::info!("{}, {}", req.method(), req.uri().path());

        let mut resp = match (req.uri().path(), req.method()) {
            ("/callback", &hyper::Method::GET) => self.twitch_callback(req).await?,
            ("/spotifycallback", &hyper::Method::GET) => self.spotify_callback(req).await?,

            ("/restart", _) => {
                let mut body = HashMap::new();
                body.insert("status", "restarting");
                let body = serde_json::to_string(&body).unwrap();

                let response = Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(body.into())
                    .expect("Failed to build response in restart");

                *self.status.0.lock().await = ServerStatus::Restarting;
                self.status.1.notify_waiters();

                return Ok(response);
            }

            ("/config", &hyper::Method::POST) => {
                let body = hyper::body::to_bytes(req.into_body()).await?;
                let new_config = match serde_json::from_slice(&body) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to parse config: {}", e);
                        let response = Response::builder()
                            .status(400)
                            .body(Body::empty())
                            .expect("Failed to build response in config POST");
                        return Ok(response);
                    }
                };

                *self.config.lock().await = new_config;
                let response = Response::builder()
                    .status(200)
                    .body(Body::empty())
                    .expect("Failed to build response");
                response
            }

            ("/config", &hyper::Method::GET) => {
                let config = self.config.lock().await;
                let response = Response::builder()
                    .status(200)
                    .body(Body::from(serde_json::to_string(&*config).unwrap()))
                    .expect("Failed to build response");

                response
            }

            ("/config", &hyper::Method::OPTIONS) => {
                let response = Response::builder()
                    .status(200)
                    .body(Body::empty())
                    .expect("Failed to build response");
                response
            }

            ("/saveconfig", &hyper::Method::POST) => {
                let config = self.config.lock().await;
                match config.save_to_file() {
                    Ok(_) => {
                        let response = Response::builder()
                            .status(200)
                            .body(Body::empty())
                            .expect("Failed to build response");
                        response
                    }
                    Err(e) => {
                        tracing::error!("Failed to save config: {}", e);
                        let response = Response::builder()
                            .status(500)
                            .body(Body::empty())
                            .expect("Failed to build response");
                        response
                    }
                }
            }

            ("/status", &hyper::Method::GET) => {
                let mut sys = sysinfo::System::new_all();
                sys.refresh_all();

                let status = self.system_status.lock().await.clone();

                let response = Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&status).unwrap()))
                    .expect("Failed to build response");
                response
            }

            _ => {
                let response = Response::builder()
                    .status(404)
                    .body(Body::empty())
                    .expect("Failed to build response");
                response
            }
        };

        resp.headers_mut()
            .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
        resp.headers_mut().insert(
            "Access-Control-Allow-Methods",
            "GET, POST, OPTIONS".parse().unwrap(),
        );
        resp.headers_mut().insert(
            "Access-Control-Allow-Headers",
            "Content-Type".parse().unwrap(),
        );

        let mut cr = self.callback_response.lock().await;

        if cr.delivered && !cr.ack {
            tracing::info!("All auth codes received, starting bot.");
            self.callback_completed.lock().await.notify_waiters();
            cr.ack = true;
        }

        Ok(resp)
    }

    async fn twitch_callback(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        if self.callback_response.lock().await.delivered {
            return Ok(Response::new(Body::from(
                "You have already authenticated. You can close this tab now.",
            )));
        }

        let query_params: Vec<_> = req.uri().query().unwrap_or("").split('&').collect();
        let mut code = None;
        let mut error = None;
        let mut error_description = None;

        let mut query_keys = Vec::new();

        for param in query_params {
            let key_value: Vec<_> = param.splitn(2, '=').collect();
            if key_value.len() == 2 {
                query_keys.push(key_value[0]);
                match key_value[0] {
                    "code" => code = Some(key_value[1].to_owned()),

                    "error" => error = Some(key_value[1].to_owned()),
                    "error_description" => error_description = Some(key_value[1].to_owned()),
                    _ => {}
                }
            }
        }

        if let Some(error) = error {
            tracing::error!(
                "Twitch error: {} {}",
                error,
                error_description.unwrap_or_default()
            );

            return self.twitch_callback_error(req).await;
        }

        if let Some(code) = code {
            tracing::info!("Twitch Auth Code Received.");
            let twitch_auth = match twitch::exchange_code(self.config.clone(), code).await {
                Ok(a) => a,
                Err(e) => {
                    match e.kind {
                        ErrorKind::TwitchError => {
                            tracing::error!("Unable to authenticate with Twitch. Please try again. Ensure to use the new link provided, refreshing the callback page will not work.");
                            return self.twitch_callback_error(req).await;
                        }
                        _ => {
                            tracing::error!(
                                "An error occured exchanging twitch auth code for an access code: {}",
                                e.message
                            );
                            return self.twitch_callback_error(req).await;
                        }
                    };
                }
            };

            self.callback_response
                .lock()
                .await
                .twitch_auth
                .replace(twitch_auth);

            if self.config.lock().await.spotify_enabled {
                let mut cr = self.callback_response.lock().await;
                if cr.spotify_auth.is_some() {
                    cr.delivered = true;
                    self.callback_completed.lock().await.notify_waiters();
                }
            } else {
                self.callback_response.lock().await.delivered = true;
            }

            let response =
                Response::new(Body::from("Callback Received! You can close this tab now."));
            return Ok(response);
        } else {
            tracing::error!(
                "Invalid Callback Response. Expected code or error query keys. Recieved {:?}",
                query_keys
            );
            let response = Response::new(Body::from(
                "Invalid Callback Response. Check the terminal for more information.",
            ));
            return Ok(response);
        }
    }

    async fn spotify_callback(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        if self.callback_response.lock().await.delivered {
            return Ok(Response::new(Body::from(
                "You have already authenticated. You can close this tab now.",
            )));
        }

        let query_params = req.uri().query().unwrap_or("");
        let mut code = None;
        let mut error = None;

        let mut query_keys = Vec::new();

        for param in query_params.split('&') {
            let key_value: Vec<&str> = param.splitn(2, '=').collect();
            if key_value.len() == 2 {
                query_keys.push(key_value[0]);
                match key_value[0] {
                    "code" => code = Some(key_value[1].to_owned()),
                    "error" => error = Some(key_value[1].to_owned()),
                    _ => {}
                }
            }
        }

        if let Some(error) = error {
            tracing::error!("Spotify error: {}", error);
            return Ok(Response::new(Body::from(
                "An error has occured. Check the terminal for more information.",
            )));
        }

        if let Some(code) = code {
            tracing::info!("Spotify Auth Code Received.");
            let conf = self.config.lock().await;
            let client_id = conf
                .spotify_client_id
                .clone()
                .expect("SPOTIFY_CLIENT_ID is not set");
            let client_secret = conf
                .spotify_client_secret
                .clone()
                .expect("SPOTIFY_CLIENT_SECRET is not set");

            let spotify_auth = match spotify::exchange_code(code, &client_id, &client_secret).await
            {
                Ok(a) => a,
                Err(e) => {
                    match e.kind {
                        ErrorKind::SpotifyError => {
                            tracing::error!("Unable to authenticate with Spotify. Please try again. Ensure to use the new link provided, refreshing the callback page will not work.");
                            return self.spotify_callback_error(req).await;
                        }
                        _ => {
                            tracing::error!(
                                "An error occured exchanging spotify auth code for an access code: {}",
                                e.message
                            );
                            return self.spotify_callback_error(req).await;
                        }
                    };
                }
            };

            self.callback_response
                .lock()
                .await
                .spotify_auth
                .replace(spotify_auth);

            let mut cr = self.callback_response.lock().await;
            if cr.twitch_auth.is_some() {
                cr.delivered = true;
                self.callback_completed.lock().await.notify_waiters();
            }

            let response =
                Response::new(Body::from("Callback Received! You can close this tab now."));
            return Ok(response);
        } else {
            tracing::error!(
                "Invalid Callback Response. Expected code or error query key. Recieved {:?}",
                query_keys
            );
            let response = Response::new(Body::from(
                "Invalid Callback Response. Check the terminal for more information.",
            ));
            return Ok(response);
        }
    }

    async fn twitch_callback_error(
        &self,
        _req: Request<Body>,
    ) -> Result<Response<Body>, hyper::Error> {
        let html = include_str!("./static/callback_error.html");
        let html = html.replace(
            "{{ .RedirectURL }}",
            twitch::make_oauth_url(
                &self.config.lock().await.twitch_client_id,
                twitch::CALLBACK_URI,
            )
            .as_str(),
        );

        let response = Response::builder()
            .status(400)
            .header("Content-Type", "text/html")
            .body(Body::from(html))
            .expect("Failed to build response in callback_error");

        return Ok(response);
    }

    async fn spotify_callback_error(
        &self,
        _req: Request<Body>,
    ) -> Result<Response<Body>, hyper::Error> {
        let client_id = match self.config.lock().await.spotify_client_id.clone() {
            Some(c) => c,
            None => {
                tracing::error!("Failed to get spotify client id");
                let response = Response::builder()
                    .status(400)
                    .header("Content-Type", "text/html")
                    .body(Body::from(
                        "An error has occured. Check the terminal for more information.",
                    ))
                    .expect("Failed to build response in spotify_callback_error");

                return Ok(response);
            }
        };

        let html = include_str!("./static/callback_error.html");
        let html = html.replace(
            "{{ .RedirectURL }}",
            spotify::make_oauth_url(&client_id, spotify::CALLBACK_URI).as_str(),
        );

        let response = Response::builder()
            .status(400)
            .header("Content-Type", "text/html")
            .body(Body::from(html))
            .expect("Failed to build response in callback_error");

        return Ok(response);
    }
}
