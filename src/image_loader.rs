use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{Error, ErrorKind, Read, Result},
    path::{Path, PathBuf},
};

use crate::api::image::ImageRequest;

pub trait ImageLoader {
    fn get_image(
        &self,
        prefix: &OsStr,
        request: &ImageRequest,
    ) -> Result<Box<dyn Read>>;
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
    ) -> Result<Box<dyn Read>> {
        let dir = PathBuf::from(
            self.image_dirs
                .get(prefix)
                .ok_or(Error::from(ErrorKind::NotFound))?,
        );
        let mut file_name = OsString::from(&request.identifier);
        file_name.push(".tif");
        let file_path = find_file(&dir, &file_name)?;
        Ok(Box::new(File::open(&file_path)?))
    }
}

fn find_file(dir: &Path, file_name: &OsStr) -> Result<PathBuf> {
    let mut file_path = PathBuf::from(dir);
    file_path.push(file_name);
    if file_path.exists() {
        if file_path.is_file() {
            Ok(file_path)
        } else {
            Err(Error::from(ErrorKind::InvalidFilename))
        }
    } else {
        Err(Error::from(ErrorKind::NotFound))
    }
}
