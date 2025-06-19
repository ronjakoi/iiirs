use image::{DynamicImage, ImageReader};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::{Error, ErrorKind, Result},
    path::PathBuf,
};

use crate::api::image::ImageRequest;

const ON_DISK_FORMAT_EXT: &'static str = "tif";

pub trait ImageLoader {
    fn get_image(
        &self,
        prefix: &OsStr,
        request: &ImageRequest,
    ) -> Result<DynamicImage>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct LocalLoader {
    image_dirs: HashMap<OsString, OsString>,
}

impl LocalLoader {
    pub fn new() -> Self {
        Self {
            image_dirs: HashMap::new(),
        }
    }

    pub fn insert<S: Into<OsString>>(&mut self, prefix: S, dir: S) {
        self.image_dirs.insert(prefix.into(), dir.into());
    }
}

impl<S: Into<OsString>> FromIterator<(S, S)> for LocalLoader {
    fn from_iter<T: IntoIterator<Item = (S, S)>>(iter: T) -> Self {
        let image_dirs = iter
            .into_iter()
            .map(|(key, val)| (key.into(), val.into()))
            .collect();
        Self { image_dirs }
    }
}

impl ImageLoader for LocalLoader {
    fn get_image(
        &self,
        prefix: &OsStr,
        request: &ImageRequest,
    ) -> Result<DynamicImage> {
        let dir = OsString::from(
            self.image_dirs
                .get(prefix)
                .ok_or(Error::from(ErrorKind::NotFound))?,
        );
        let mut file_path = PathBuf::with_capacity(
            &dir.len()
                + &request.identifier.as_os_str().len()
                + ".".len()
                + ON_DISK_FORMAT_EXT.len(),
        );
        file_path.push(&dir);
        file_path.push(&request.identifier);
        file_path.set_extension(ON_DISK_FORMAT_EXT);
        let image = ImageReader::open(&file_path)?.decode().unwrap();
        Ok(image)
    }
}
