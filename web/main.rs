mod files;

use std::sync::Arc;

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(short, long)]
    internal_port: Option<u16>,
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    {
        ansi_term::enable_ansi_support().unwrap();
    }

    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let api = Arc::new(Api::new(
        args.port.unwrap_or(8080),
        args.internal_port.unwrap_or(3000),
    ));

    hyper_server(api).await;
}

pub struct Api {
    port: u16,
    internal_port: u16,
}

impl Api {
    fn new(port: u16, internal_port: u16) -> Self {
        Self {
            port,
            internal_port,
        }
    }
}

async fn hyper_server(api: Arc<Api>) {
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

    let addr = ([127, 0, 0, 1], api.port).into();
    let server = Server::bind(&addr).serve(make_svc);

    tracing::info!("Listening on http://{}", addr);
    server.await.unwrap();
}

impl Api {
    async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
        tracing::info!("{}, {}", req.method(), req.uri().path());
        match match (req.uri().path(), req.method()) {
            ("/", &hyper::Method::GET) => self.index(req).await,
            ("/settings", &hyper::Method::GET) => self.settings(req).await,
            ("/js/index.js", &hyper::Method::GET) => self.file_serve(req).await,
            ("/js/settings.js", &hyper::Method::GET) => self.file_serve(req).await,
            ("/css/styles.css", &hyper::Method::GET) => self.file_serve(req).await,
    
            _ => self.not_found().await,
        } {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::error!("Failed to handle request: {}", e);
                Ok(Response::builder()
                    .status(500)
                    .body(Body::empty())
                    .expect("Failed to build response in handle_request"))
            }
        }
    }

    pub async fn not_found(&self) -> Result<Response<Body>, Box<dyn std::error::Error>> {
        match Response::builder().status(404).body(Body::empty()) {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("Failed to build response: {}", e).into()),
        }
    }
    
    pub async fn file_serve(&self, req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
        let path = req.uri().path();
    
        let response = match path {
            "/js/index.js" => {
                let js = files::INDEX_JS.replace("{{ .PORT }}", &self.internal_port.to_string());

                Response::builder()
                .status(200)
                .header("Content-Type", "text/javascript")
                .body(Body::from(js))?
            }
            "/js/settings.js" => {
                let js = files::SETTINGS_JS.replace("{{ .PORT }}", &self.internal_port.to_string());

                Response::builder()
                .status(200)
                .header("Content-Type", "text/javascript")
                .body(Body::from(js))?
            }
            "/css/styles.css" => Response::builder()
                .status(200)
                .header("Content-Type", "text/css")
                .body(Body::from(files::STYLES_CSS))?,
            _ => self.not_found().await?,
        };
    
        Ok(response)
    }
    
    pub async fn index(&self, _req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
        let response = Response::builder()
            .status(200)
            .header("Content-Type", "text/html")
            .body(Body::from(files::INDEX_HTML))?;
    
        Ok(response)
    }
    
    pub async fn settings(&self, _req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
        let response = Response::builder()
            .status(200)
            .header("Content-Type", "text/html")
            .body(Body::from(files::SETTINGS_HTML))?;
    
        Ok(response)
    }
    
}