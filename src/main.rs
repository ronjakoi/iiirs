use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, status::StatusCode};
use axum::response::ErrorResponse;
use axum::{
    Router,
    extract::{Path, State},
    response::Result,
    routing::get,
};
use tokio::sync::RwLock;

use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

mod api;
mod image_loader;
mod image_ops;
use api::image::{ImageRequest, Region, Rotation, Size};
use image_loader::{GenericImageLoader, ImageLoader, LocalLoader};
use image_ops::{crop_image, resize_image, rotate_image};

use crate::image_loader::ProxyLoader;

const DEFAULT_USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), " v", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
struct AppState {
    image_loaders: HashMap<String, Arc<RwLock<ImageLoader>>>,
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

    let loader = app_state
        .image_loaders
        .get(&prefix)
        .ok_or(StatusCode::NOT_FOUND)?
        .read()
        .await;

    let mut image = loader
        .get_image(&prefix, &req)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if req.region != Region::default() {
        image = crop_image(image, &req.region);
    }

    if req.size != Size::default() {
        image = resize_image(image, &req.size)?;
    }

    if req.rotation != Rotation::default() {
        rotate_image(&mut image, &req.rotation);
    }

    let mut image_data = Cursor::new(vec![]);
    image
        .write_to(&mut image_data, req.format)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        req.format
            .to_mime_type()
            .parse()
            .expect("failed to parse mime type"),
    );

    Ok((headers, image_data.into_inner()))
}

#[tokio::main]
async fn main() {
    let local = ImageLoader::Local(LocalLoader::from_iter([("test", "./")]));
    let proxy = ImageLoader::Proxy(ProxyLoader::new("proxy", "./proxy_cache"));
    let state = AppState {
        image_loaders: HashMap::from([
            (String::from("test"), Arc::new(RwLock::new(local))),
            (String::from("proxy"), Arc::new(RwLock::new(proxy))),
        ]),
    };
    let app = Router::new()
        .route("/iiif/{prefix}/{*image_request}", get(get_image))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
