mod files;

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    {
        ansi_term::enable_ansi_support().unwrap();
    }

    tracing_subscriber::fmt::init();

    hyper_server().await;
}

async fn hyper_server() {
    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, hyper::Error>(service_fn(handle_request)) });

    let addr = ([127, 0, 0, 1], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);

    tracing::info!("Listening on http://{}", addr);
    server.await.unwrap();
}

async fn not_found() -> Result<Response<Body>, Box<dyn std::error::Error>> {
    match Response::builder().status(404).body(Body::empty()) {
        Ok(response) => Ok(response),
        Err(e) => Err(format!("Failed to build response: {}", e).into()),
    }
}

async fn file_serve(req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
    let path = req.uri().path();

    let response = match path {
        "/js/index.js" => Response::builder()
            .status(200)
            .header("Content-Type", "text/javascript")
            .body(Body::from(files::INDEX_JS))?,
        "/js/settings.js" => Response::builder()
            .status(200)
            .header("Content-Type", "text/javascript")
            .body(Body::from(files::SETTINGS_JS))?,
        "/css/styles.css" => Response::builder()
            .status(200)
            .header("Content-Type", "text/css")
            .body(Body::from(files::STYLES_CSS))?,
        _ => not_found().await?,
    };

    Ok(response)
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    tracing::info!("{}, {}", req.method(), req.uri().path());
    match match (req.uri().path(), req.method()) {
        ("/", &hyper::Method::GET) => index(req).await,
        ("/settings", &hyper::Method::GET) => settings(req).await,
        ("/js/index.js", &hyper::Method::GET) => file_serve(req).await,
        ("/js/settings.js", &hyper::Method::GET) => file_serve(req).await,
        ("/css/styles.css", &hyper::Method::GET) => file_serve(req).await,

        _ => not_found().await,
    } {
        Ok(mut response) => {
            response.headers_mut().insert(
                "Access-Control-Allow-Origin",
                "http://127.0.0.1:3000".parse().unwrap(),
            );
            response.headers_mut().insert(
                "Access-Control-Allow-Methods",
                "GET, POST, OPTIONS".parse().unwrap(),
            );
            response.headers_mut().insert(
                "Access-Control-Allow-Headers",
                "Content-Type".parse().unwrap(),
            );
            Ok(response)
        }
        Err(e) => {
            tracing::error!("Failed to handle request: {}", e);
            Ok(Response::builder()
                .status(500)
                .body(Body::empty())
                .expect("Failed to build response in handle_request"))
        }
    }
}

async fn index(_req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
    let response = Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(files::INDEX_HTML))?;

    Ok(response)
}

async fn settings(_req: Request<Body>) -> Result<Response<Body>, Box<dyn std::error::Error>> {
    let response = Response::builder()
        .status(200)
        .header("Content-Type", "text/html")
        .body(Body::from(files::SETTINGS_HTML))?;

    Ok(response)
}
