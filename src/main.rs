use axum::http::status::StatusCode;
use axum::{Router, extract::Path, response::IntoResponse, routing::get};
use std::fs::File;
use std::path::PathBuf;

mod api;
mod image_loader;
use api::image::*;
use image_loader::LocalLoader;

async fn get_image(
    Path((prefix, image_request)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    let req: ImageRequest =
        image_request.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut img_file = PathBuf::from(&prefix);
    img_file.push(&req.identifier);

    Ok(format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n",
        &prefix,
        &req.identifier,
        req.region,
        req.size,
        req.rotation,
        req.quality,
        req.format
    ))
}

#[tokio::main]
async fn main() {
    let loader = LocalLoader::from_iter([("test", "./")].into_iter());
    let app =
        Router::new().route("/iiif/{prefix}/{*image_request}", get(get_image));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
