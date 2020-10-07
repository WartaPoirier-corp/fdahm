use super::fdahm_result::{FdahmError, FdahmResult};

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml_edit::value;

const VIDEO_TOML: &str = "video.toml";

#[derive(Debug, serde::Deserialize)]
pub struct GlobalConfig {
    pub channel_id: u64,
    pub name: String,
    pub pp_url: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VideoMeta {
    pub title: String,
    pub views: u64,

    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub published: bool,
}

#[derive(Debug)]
pub struct Video {
    pub id: String,
    pub meta: VideoMeta,
}

fn read_toml<T: serde::de::DeserializeOwned>(file: &Path) -> FdahmResult<T> {
    let content = fs::read_to_string(file).map_err(|e| FdahmError::CannotRead {
        base: e,
        path: file.to_owned(),
    })?;
    toml::from_str(&content).map_err(|e| FdahmError::MalformedToml {
        base: e,
        path: file.to_owned(),
    })
}

pub struct FdahmDirectory<'a> {
    dir: &'a Path,
}

impl<'a> FdahmDirectory<'a> {
    pub fn new(dir: &'a Path) -> FdahmResult<Self> {
        if !dir.is_dir() {
            panic!("{:?} is not a directory", dir);
        }

        Ok(Self { dir })
    }

    pub fn global_config(&self) -> FdahmResult<GlobalConfig> {
        read_toml(&self.dir.join("fdahm.toml"))
    }

    pub fn get_video_by_id(&self, id: String) -> FdahmResult<Video> {
        let path = self.dir.join(&id);

        Ok(Video {
            id,
            meta: read_toml(&path.join(VIDEO_TOML))?,
        })
    }

    pub fn get_thumbnail(&self, video: &Video) -> FdahmResult<PathBuf> {
        let path = self.dir.join(&video.id);

        let thumbnail_png = path.join("thumbnail.png");
        let thumbnail_jpg = path.join("thumbnail.jpg");

        let thumbnail_png_exists = thumbnail_png.exists();
        let thumbnail_jpg_exists = thumbnail_jpg.exists();

        match (thumbnail_png_exists, thumbnail_jpg_exists) {
            (true, true) => Err(FdahmError::AmbiguousThumbnail(video.id.to_owned())),
            (false, false) => Err(FdahmError::NoThumbnail(video.id.to_owned())),
            (true, false) => Ok(thumbnail_png),
            (false, true) => Ok(thumbnail_jpg),
        }
    }

    pub fn list_videos(&self) -> FdahmResult<Vec<Video>> {
        let dirs = fs::read_dir(self.dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                if let Ok(meta) = e.metadata() {
                    meta.is_dir()
                } else {
                    false
                }
            })
            .map(|e| e.file_name());

        let videos = dirs
            .map(|name| self.get_video_by_id(name.to_string_lossy().to_string()))
            .collect::<Result<_, _>>();

        videos
    }

    pub fn mark_published(&self, video: &Video) -> FdahmResult<()> {
        let path = self.dir.join(&video.id).join("video.toml");
        let file = fs::read_to_string(&path).unwrap();
        let mut doc = toml_edit::Document::from_str(&file).unwrap();

        doc.root["published"] = value(true);
        // doc.root["published_date"] = value(Datetime::OffsetDateTime(Utc::now())) TODO

        std::fs::write(&path, doc.to_string()).map_err(|_| FdahmError::CannotMarkPublished)
    }
}
