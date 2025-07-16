use image::DynamicImage;
use serde::Serialize;

static TYPE: &str = "ImageService3";
static IMAGE_3_CONTEXT: &str = "http://iiif.io/api/image/3/context.json";
static PROTOCOL: &str = "http://iiif.io/api/image";

const MAX_WIDTH: u32 = 10_000;
const MAX_HEIGHT: u32 = 10_000;
const MAX_AREA: u64 = 50_000_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    #[serde(rename = "@context")]
    context: Vec<String>,
    id: String,
    #[serde(rename = "type")]
    type_: &'static str,
    protocol: &'static str,
    profile: ComplianceLevel,
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
    max_area: u64,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
enum ComplianceLevel {
    Level0,
    Level1,
    #[default]
    Level2,
}

impl ImageInfo {
    pub fn new(prefix: &str, id: &str, image: &DynamicImage) -> Self {
        let id = ["http://localhost:3000/iiif", prefix, id].join("/");
        Self {
            context: vec![IMAGE_3_CONTEXT.into()],
            id,
            type_: TYPE,
            protocol: PROTOCOL,
            profile: ComplianceLevel::Level2,
            width: image.width(),
            height: image.height(),
            max_width: MAX_WIDTH,
            max_height: MAX_HEIGHT,
            max_area: MAX_AREA,
        }
    }
}
