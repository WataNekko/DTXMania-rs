pub mod song;

use std::{
    collections::HashMap,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_fs::File;
use async_lock::OnceCell;
use bevy::{
    asset::io::{
        AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, ErasedAssetReader,
        PathStream, Reader,
    },
    prelude::*,
    tasks::futures_lite::StreamExt,
};
use dashmap::DashMap;

pub struct DtxAssetPlugin;

impl Plugin for DtxAssetPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_source(
            DTX_SOURCE_ID,
            AssetSourceBuilder::platform_default("", None)
                .with_reader(|| Box::new(DtxAssetReader::new())),
        );
    }
}

pub const DTX_SOURCE_ID: &str = "dtx";

pub struct DtxAssetReader {
    inner: Box<dyn ErasedAssetReader>,
    cache: DirNoCaseCache,
}

impl DtxAssetReader {
    pub fn new() -> Self {
        Self {
            inner: AssetSource::get_default_reader(String::new())(),
            cache: DirNoCaseCache(DashMap::new()),
        }
    }
}

impl AssetReader for DtxAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        match self.inner.read(path).await {
            err @ Err(AssetReaderError::NotFound(_)) => {
                // Try to look for the file ignoring case using async_fs::read_dir and
                // async_fs::File directly.
                //
                // Can't utilize AssetReader::read and read_directory here (the cross-platform
                // implementations) because the lifetime 'a don't allow us to return these trait
                // functions' results from here using a &'b Path input from some other source. The
                // actual path with the different case (if any) is either cached somewhere else or
                // created in this function, so can't satitfy lifetime 'a.
                let Some(dir) = path.parent() else {
                    return err;
                };
                let Some(file_name) = path.file_name() else {
                    return err;
                };

                let dir_no_case_cache = self
                    .cache
                    .0
                    .entry(dir.to_owned())
                    .or_insert_with(|| Arc::new(OnceCell::new()))
                    .clone();

                let dir_no_case_cache = dir_no_case_cache
                    .as_ref()
                    .get_or_init(async || {
                        // cache the dir once
                        let dir_entries = async_fs::read_dir(dir).await.ok()?;

                        let cache = dir_entries
                            .filter_map(|entry| entry.ok())
                            .map(|entry| {
                                let mut name_no_case = entry.file_name();
                                name_no_case.make_ascii_lowercase();
                                (name_no_case, entry.path())
                            })
                            .collect()
                            .await;

                        Some(cache)
                    })
                    .await;

                let Some(real_path) = dir_no_case_cache
                    .as_ref()
                    .and_then(|cache| cache.get(&file_name.to_ascii_lowercase()))
                else {
                    return err;
                };

                match File::open(real_path).await {
                    Ok(file) => Ok(Box::new(file)),
                    Err(e) if e.kind() == ErrorKind::NotFound => err,
                    Err(e) => Err(e.into()),
                }
            }

            res => res,
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        self.inner.read_meta(path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}

struct DirNoCaseCache(DashMap<PathBuf, Arc<OnceCell<Option<HashMap<OsString, PathBuf>>>>>);
