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
    tasks::{IoTaskPool, futures_lite::StreamExt},
};
use dashmap::DashMap;

pub struct DtxAssetPlugin;

impl Plugin for DtxAssetPlugin {
    fn build(&self, app: &mut App) {
        let case_result_handle = Arc::new(OnceCell::new());

        app.add_systems(Startup, check_case_sensitivity(case_result_handle.clone()))
            .register_asset_source(
                DTX_SOURCE_ID,
                AssetSourceBuilder::platform_default("", None).with_reader(move || {
                    Box::new(DtxAssetReader {
                        inner: AssetSource::get_default_reader(String::new())(),
                        cache: DirNoCaseCache(DashMap::new()),
                        case_sensitive: case_result_handle.clone(),
                    })
                }),
            );
    }
}

pub const DTX_SOURCE_ID: &str = "dtx";

fn check_case_sensitivity(case_sensitive: Arc<OnceCell<bool>>) -> impl Fn() {
    move || {
        let case_result_handle = case_sensitive.clone();

        IoTaskPool::get()
            .spawn(async move {
                case_result_handle.get_or_init(is_case_sensitive_fs).await;
            })
            .detach();
    }
}

async fn is_case_sensitive_fs() -> bool {
    const UPPER_FILE_NAME: &str = ".DTX_TEST_CASE.tmp";
    const LOWER_FILE_NAME: &str = ".dtx_test_case.tmp";

    let _ = async_fs::remove_file(UPPER_FILE_NAME).await;
    let _ = async_fs::remove_file(LOWER_FILE_NAME).await;

    if File::create(UPPER_FILE_NAME).await.is_err() {
        return true; // to be sure that we'll run the ignore case check anyway
    }

    let is_case_sensitive = async_fs::metadata(LOWER_FILE_NAME).await.is_err();

    let _ = async_fs::remove_file(UPPER_FILE_NAME).await;

    is_case_sensitive
}

struct DtxAssetReader {
    inner: Box<dyn ErasedAssetReader>,
    cache: DirNoCaseCache,
    case_sensitive: Arc<OnceCell<bool>>,
}

impl AssetReader for DtxAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        match self.inner.read(path).await {
            not_found @ Err(AssetReaderError::NotFound(_))
                if *self.case_sensitive.get_or_init(is_case_sensitive_fs).await =>
            {
                // If can't find the file on a case sensitive FS, try looking again for the file
                // name ignoring case, using async_fs::read_dir to traverse the directory.
                //
                // Can't utilize AssetReader::read and read_directory here (the cross-platform
                // implementations) because the lifetime 'a don't allow us to return these trait
                // functions' results from here using a &'b Path input from some other source. The
                // actual path with the different case (if any) is either cached somewhere else or
                // created in this function, so can't satitfy lifetime 'a.
                let Some(dir) = path.parent() else {
                    return not_found;
                };
                let Some(file_name) = path.file_name() else {
                    return not_found;
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
                    return not_found;
                };

                match File::open(real_path).await {
                    Ok(file) => Ok(Box::new(file)),
                    Err(e) if e.kind() == ErrorKind::NotFound => not_found,
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
