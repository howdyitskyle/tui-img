use crate::models::CachedImageInfo;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn get_file_mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

pub fn get_cached_metadata(
    metadata_cache: &HashMap<PathBuf, CachedImageInfo>,
    path: &Path,
) -> Option<CachedImageInfo> {
    let cached = metadata_cache.get(path)?;
    let current_mtime = get_file_mtime(path);
    if current_mtime != 0 && cached.file_mtime == current_mtime {
        Some(cached.clone())
    } else {
        None
    }
}

pub fn cache_metadata(
    metadata_cache: &mut HashMap<PathBuf, CachedImageInfo>,
    path: PathBuf,
    info: CachedImageInfo,
) {
    metadata_cache.insert(path, info);
}

pub fn get_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str());
    let mut counter = 1;

    loop {
        let new_name = match ext {
            Some(e) => format!("{}_{}.{}", stem, counter, e),
            None => format!("{}_{}", stem, counter),
        };
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}
