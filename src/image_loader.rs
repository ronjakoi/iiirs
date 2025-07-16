use axum::http::{HeaderName, header};
use base64ct::{Base64UrlUnpadded, Encoding};
use image::{DynamicImage, ImageFormat, ImageReader};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::{Cursor, Error, ErrorKind, Result},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};
use walkdir::WalkDir;

use crate::DEFAULT_USER_AGENT;

const ON_DISK_FORMAT_EXT: &str = "tif";

// The AppState contains a HashMap over all loaders, and because get_image() is
// async, GenericImageLoader is not a dyn-compatible trait. This enum is a
// work-around for that.
#[derive(Debug)]
pub enum ImageLoader {
    Local(LocalLoader),
    Proxy(ProxyLoader),
}

pub trait GenericImageLoader {
    async fn get_image(
        &mut self,
        prefix: &str,
        identifier: &str,
    ) -> Result<DynamicImage>;
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct LocalLoader {
    image_dirs: HashMap<String, PathBuf>,
}

type Sha256Bytes = [u8; 32];
type ContentCacheKey = Sha256Bytes;

#[derive(Debug, Default)]
pub struct ProxyLoader {
    cache_dir: PathBuf,
    // TODO: move this to sqlite or redis or something
    uri_to_hash_key: HashMap<String, (Sha256Bytes, ImageFormat)>,
    client: reqwest::Client,
}

impl GenericImageLoader for ImageLoader {
    async fn get_image(
        &mut self,
        prefix: &str,
        identifier: &str,
    ) -> Result<DynamicImage> {
        match self {
            Self::Local(local) => local.get_image(prefix, identifier).await,
            Self::Proxy(proxy) => proxy.get_image(prefix, identifier).await,
        }
    }
}

impl LocalLoader {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn insert_dir<S, T>(&mut self, prefix: S, dir: T)
    where
        S: Into<String>,
        T: Into<PathBuf>,
    {
        self.image_dirs.insert(prefix.into(), dir.into());
    }
}

impl<S, Z> FromIterator<(S, Z)> for LocalLoader
where
    S: Into<String>,
    Z: Into<PathBuf>,
{
    fn from_iter<T: IntoIterator<Item = (S, Z)>>(iter: T) -> Self {
        let image_dirs = iter
            .into_iter()
            .map(|(key, val)| (key.into(), val.into()))
            .collect();
        Self { image_dirs }
    }
}

impl GenericImageLoader for LocalLoader {
    async fn get_image(
        &mut self,
        prefix: &str,
        identifier: &str,
    ) -> Result<DynamicImage> {
        let dir = OsString::from(
            self.image_dirs
                .get(prefix)
                .ok_or(Error::from(ErrorKind::NotFound))?,
        );
        let mut file_path = PathBuf::with_capacity(
            dir.len() + identifier.len() + ".".len() + ON_DISK_FORMAT_EXT.len(),
        );
        file_path.push(&dir);
        file_path.push(&identifier);
        file_path.set_extension(ON_DISK_FORMAT_EXT);
        let image =
            ImageReader::open(&file_path)?.decode().unwrap_or_else(|_| {
                panic!(
                    "LocalLoader: failed to decode image file {file_path:?}",
                )
            });
        Ok(image)
    }
}

impl ProxyLoader {
    pub fn new<T: Into<PathBuf>>(prefix: &str, path: T) -> Self {
        let cache_dir: PathBuf = path.into();
        let client = reqwest::ClientBuilder::new()
            .user_agent(DEFAULT_USER_AGENT)
            .connect_timeout(Duration::from_millis(2000))
            .read_timeout(Duration::from_millis(1000))
            .build()
            .expect("ProxyLoader: failed to initialize http client");
        let mut local_loader = LocalLoader::new();
        for path in get_leaf_dirs(&cache_dir) {
            local_loader.insert_dir(prefix, path);
        }

        tracing::debug!(
            "Initialized ProxyLoader for prefix {} with cache dir in {:?}",
            prefix,
            &cache_dir
        );

        Self {
            cache_dir,
            client,
            ..Default::default()
        }
    }

    fn get_from_cache(
        &self,
        key: &ContentCacheKey,
        format: ImageFormat,
    ) -> Option<DynamicImage> {
        let path = cached_img_path(&self.cache_dir, key);
        match ImageReader::open(&path) {
            Ok(mut reader) => {
                reader.set_format(format);
                let image = reader.decode().unwrap_or_else(|_| {
                    panic!(
                        "ProxyLoader: {path:?} found in cache but failed to decode",
                    )
                });
                Some(image)
            }
            Err(_) => None,
        }
    }

    async fn get_from_uri(
        &self,
        uri: &str,
    ) -> Option<(DynamicImage, ImageFormat)> {
        let response = self.client.get(uri).send().await.unwrap();
        match response.status() {
            StatusCode::OK => {
                tracing::debug!("ProxyLoader: response OK for {}", &uri);
                let mime = response
                    .headers()
                    .get(HeaderName::from(header::CONTENT_TYPE));
                let mime = mime.as_deref();
                let format = if let Some(mime) = mime {
                    ImageFormat::from_mime_type(mime.to_str().unwrap())
                } else {
                    let url = response.url();
                    let filename: String = url
                        .path_segments()
                        .iter()
                        .last()
                        .unwrap()
                        .clone()
                        .collect();
                    let ext = filename.split('.').last().unwrap();
                    ImageFormat::from_extension(ext)
                }
                .unwrap();

                tracing::debug!(
                    "Proxyloader: parsed image format {:?}",
                    &format
                );

                let data = response.bytes().await.unwrap();
                let mut reader = ImageReader::new(Cursor::new(data));
                reader.set_format(format);

                Some((reader.decode().unwrap(), format))
            }
            _ => None,
        }
    }

    async fn write_in_cache(
        &mut self,
        image: &DynamicImage,
        uri: String,
        format: ImageFormat,
    ) -> Result<()> {
        use std::io::{Error, ErrorKind, Result};

        let mut sha256 = Sha256::new();
        sha256.update(&image.as_bytes());
        let content_hash: ContentCacheKey = sha256.finalize().into();

        let cache_path = cached_img_path(&self.cache_dir, &content_hash);

        if cache_path.exists() {
            Result::Err(Error::new(
                ErrorKind::AlreadyExists,
                "Cache file already exists",
            ))
        } else {
            let leaf_dir = cache_path.parent().unwrap();
            std::fs::create_dir_all(leaf_dir)?;
            image.save_with_format(cache_path, format).unwrap();

            self.uri_to_hash_key.insert(uri, (content_hash, format));
            Result::Ok(())
        }
    }
}

impl GenericImageLoader for ProxyLoader {
    async fn get_image(
        &mut self,
        _prefix: &str,
        identifier: &str,
    ) -> Result<DynamicImage> {
        let id = identifier.trim_end_matches('=');
        let uri = Base64UrlUnpadded::decode_vec(id)
            .map_err(|_| ErrorKind::InvalidInput)?;
        let uri =
            String::from_utf8(uri).map_err(|_| ErrorKind::InvalidInput)?;
        tracing::debug!("ProxyLoader: {} decoded to {}", &identifier, &uri);

        let image = if let Some((key, format)) = self.uri_to_hash_key.get(&uri)
        {
            tracing::debug!(
                "ProxyLoader: {} should be in cache, looking on disk",
                &identifier
            );
            self.get_from_cache(key, *format)
        } else if let Some((image, format)) = self.get_from_uri(&uri).await {
            tracing::debug!(
                "ProxyLoader: writing cache entry for {}",
                &identifier
            );
            self.write_in_cache(&image, uri, format).await?;
            Some(image)
        } else {
            tracing::debug!(
                "ProxyLoader: {} not found in cache or at source",
                &identifier
            );
            None
        };
        let err = ErrorKind::NotFound.into();
        image.ok_or(err)
    }
}

fn get_leaf_dirs<P: AsRef<Path>>(path: P) -> impl Iterator<Item = OsString> {
    WalkDir::new(path)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| {
            // TODO: log an error on inaccessible directories
            e.ok()
                .and_then(|e| {
                    if e.file_type().is_dir() {
                        Some(OsString::from(e.path()))
                    } else {
                        None
                    }
                })
                .or(None)
        })
}

fn cached_img_path(cache: &Path, key: &ContentCacheKey) -> PathBuf {
    const HEX_STR_LEN: usize = size_of::<ContentCacheKey>() * 2;
    let mut key_str: [u8; HEX_STR_LEN] = [0; HEX_STR_LEN];
    base16ct::lower::encode(key, &mut key_str).unwrap();
    let sub1 = OsStr::from_bytes(&key_str[0..2]);
    let sub2 = OsStr::from_bytes(&key_str[2..4]);
    let mut path = PathBuf::with_capacity(
        cache.as_os_str().len() + sub1.len() + sub2.len() + key_str.len(),
    );
    path.push(cache);
    path.push(sub1);
    path.push(sub2);
    path.push(OsStr::from_bytes(&key_str));
    path
}
