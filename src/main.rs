use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, status::StatusCode};
use axum::response::ErrorResponse;
use axum::{
    Router,
    extract::{Path, State},
    response::Result,
    routing::get,
};
use image::ImageEncoder;
use tokio::sync::RwLock;

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Cursor;
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
) -> Result<(axum::http::HeaderMap, Vec<u8>), ErrorResponse> {
    let req: ImageRequest =
        image_request.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut img_file = PathBuf::from(&prefix);
    img_file.push(&req.identifier);

    let os_prefix = OsString::from(&prefix);
    let loader = app_state
        .image_loaders
        .get(&os_prefix)
        .ok_or(StatusCode::NOT_FOUND)?
        .read()
        .await;

    let mut image_data = Cursor::new(vec![]);
    let image = loader
        .get_image(&os_prefix, &req)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    image
        .write_to(&mut image_data, req.format)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, req.format.to_mime_type().parse().unwrap());

    Ok((headers, image_data.into_inner()))
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
