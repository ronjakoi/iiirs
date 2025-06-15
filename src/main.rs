use axum::http::HeaderName;
use axum::http::header;
use axum::http::status::StatusCode;
use axum::response::IntoResponse;
use axum::{
    Router,
    extract::{Path, State},
    routing::get,
};
use tokio::sync::RwLock;

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

mod api;
mod image_loader;
use api::image::*;
use image_loader::{ImageLoader, LocalLoader};

#[derive(Clone)]
struct AppState {
    image_loaders:
        HashMap<OsString, Arc<RwLock<dyn ImageLoader + Send + Sync>>>,
}

#[axum::debug_handler]
async fn get_image(
    Path((prefix, image_request)): Path<(String, String)>,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let req: ImageRequest = image_request
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)
        .unwrap();

    let mut img_file = PathBuf::from(&prefix);
    img_file.push(&req.identifier);

    let os_prefix = OsString::from(&prefix);
    let loader = app_state
        .image_loaders
        .get(&os_prefix)
        .ok_or(StatusCode::NOT_FOUND)
        .unwrap()
        .read()
        .await;

    let mut image_data = vec![];
    loader
        .get_image(&os_prefix, &req)
        .unwrap()
        .read_to_end(&mut image_data)
        .unwrap();

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/tiff")],
        image_data,
    )
}

#[tokio::main]
async fn main() {
    let loader = LocalLoader::from_iter([("test", "./")]);
    let state = AppState {
        image_loaders: HashMap::from([(
            OsString::from("test"),
            Arc::new(RwLock::new(loader))
                as Arc<RwLock<dyn ImageLoader + Send + Sync>>,
        )]),
    };
    let app = Router::new()
        .route("/iiif/{prefix}/{*image_request}", get(get_image))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
