use axum::http::status::StatusCode;
use axum::{Router, extract::Path, response::IntoResponse, routing::get};

mod api;
use api::*;

async fn get_image(
    Path((prefix, image_request)): Path<(String, String)>,
) -> Result<String, StatusCode> {
    let req: ImageRequest =
        image_request.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n",
        prefix,
        req.identifier,
        req.region,
        req.size,
        req.rotation,
        req.quality,
        req.format
    ))
}

#[tokio::main]
async fn main() {
    let app =
        Router::new().route("/iiif/{prefix}/{*image_request}", get(get_image));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap()
}
