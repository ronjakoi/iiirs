use std::cmp::min;
use std::convert::Into;

use axum::http::StatusCode;
use image::{DynamicImage, imageops::FilterType, metadata::Orientation};

use crate::api::image::{Region, Rotation, Size, SizeKind};

fn scale_by_pct(int: u32, pct: f32) -> u32 {
    (f64::from(int) * f64::from(pct) / 100.0).round() as u32
}

pub fn crop_image(mut image: DynamicImage, region: Region) -> DynamicImage {
    let (x, y, w, h) = match region {
        Region::Full => return image,
        Region::Square => {
            let sq_width = min(image.width(), image.height());
            let y = sq_width - image.height() / 2;
            let x = sq_width - image.width() / 2;
            (x, y, sq_width, sq_width)
        }
        Region::Absolute { x, y, w, h } => (x, y, w.into(), h.into()),
        Region::Percent { x, y, w, h } => {
            let x_pix = scale_by_pct(image.width(), x);
            let y_pix = scale_by_pct(image.height(), y);
            let w_pix = scale_by_pct(image.width(), w);
            let h_pix = scale_by_pct(image.height(), h);
            (x_pix, y_pix, w_pix, h_pix)
        }
    };
    image.crop(x, y, w, h)
}

pub fn resize_image(
    image: DynamicImage,
    size_req: Size,
) -> Result<DynamicImage, StatusCode> {
    let filter = FilterType::Triangle;
    let (nw, nh) = match size_req.kind {
        // TODO: support upscaling to maxWidth, maxHeight, maxArea, see
        // https://iiif.io/api/image/3.0/#42-size
        SizeKind::Max => return Ok(image),
        SizeKind::Width(w) => (w.into(), image.height()),
        SizeKind::Height(h) => (image.width(), h.into()),
        SizeKind::Percent(pct) => (
            scale_by_pct(image.width(), pct),
            scale_by_pct(image.height(), pct),
        ),
        SizeKind::WidthHeight { w, h } => (w.into(), h.into()),
    };

    if !size_req.allow_upscale && nw > image.width() || nh > image.height() {
        Err(StatusCode::BAD_REQUEST)
    } else if size_req.maintain_ratio {
        Ok(image.resize(nw, nh, filter))
    } else {
        Ok(image.resize_exact(nw, nh, filter))
    }
}

pub fn rotate_image(
    image: &mut DynamicImage,
    rotation: Rotation,
) -> Result<(), StatusCode> {
    match rotation {
        Rotation {
            deg: 0 | 360,
            mirror,
        } => {
            if mirror {
                image.apply_orientation(Orientation::FlipHorizontal);
            }
        }
        Rotation { deg: 90, mirror } => {
            if mirror {
                image.apply_orientation(Orientation::FlipHorizontal)
            };
            image.apply_orientation(Orientation::Rotate90);
        }
        Rotation {
            deg: 180,
            mirror: true,
        } => image.apply_orientation(Orientation::FlipVertical),
        Rotation { deg: 180, .. } => {
            image.apply_orientation(Orientation::Rotate180);
        }
        Rotation { deg: 270, mirror } => {
            if mirror {
                image.apply_orientation(Orientation::FlipHorizontal);
            }
            image.apply_orientation(Orientation::Rotate270);
        }
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    Ok(())
}
