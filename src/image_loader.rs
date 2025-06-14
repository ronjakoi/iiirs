use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{Error, ErrorKind, Read, Result},
    path::{Path, PathBuf},
};

use crate::api::image::ImageRequest;

trait ImageLoader {
    fn get_image(
        &self,
        prefix: &OsStr,
        request: ImageRequest,
    ) -> Result<impl Read>;
}

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

impl<S> FromIterator<(S, S)> for LocalLoader
where
    S: Into<OsString>,
{
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
        request: ImageRequest,
    ) -> Result<impl Read> {
        let dir = PathBuf::from(
            self.image_dirs
                .get(prefix)
                .ok_or(Error::from(ErrorKind::NotFound))?,
        );
        let mut file_name: OsString = request.identifier.into();
        file_name.push(".tif");
        let file_path = find_file(&dir, &file_name)?;
        File::open(&file_path)
    }
}

fn find_file(dir: &Path, file_name: &OsStr) -> Result<PathBuf> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        match path.file_name() {
            Some(name) => {
                if name == file_name {
                    return Ok(path.into());
                }
            }
            None => continue,
        }
    }
    Err(Error::from(ErrorKind::NotFound))
}
