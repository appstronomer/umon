use std::{
    io,
    path::{Path, PathBuf}
};

use tokio::fs;
use serde::de::{DeserializeOwned};
use tokio_util::codec::{BytesCodec, FramedRead};


pub fn path_extend<T: AsRef<Path>>(base: &PathBuf, path: T) -> Result<PathBuf, ()> {
    let mut path_new = base.clone();
    path_new.push(path);
    if let Ok(path_new) = path_new.canonicalize() {
        if path_new.starts_with(base){
            return Ok(path_new)
        }
    }
    Err(())
}


pub async fn file_deser<T>(path: PathBuf) -> Option<Result<T, ()>>
where T: DeserializeOwned
{
    if let Ok(metadata) = fs::metadata(&path).await {
        if metadata.is_file() {
            if let Ok(text) = fs::read_to_string(path).await {
                return if let Ok(obj) = serde_json::from_str::<T>(&text) {
                    Some(Ok(obj))
                } else {
                    Some(Err(()))
                };
            }
        }
    }
    None
}