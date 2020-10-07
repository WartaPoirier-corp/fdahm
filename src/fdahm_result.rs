use std::path::PathBuf;

#[derive(Debug)]
pub enum FdahmError {
    CannotRead {
        base: std::io::Error,
        path: PathBuf,
    },
    MalformedToml {
        base: toml::de::Error,
        path: PathBuf,
    },
    NoThumbnail(String),
    AmbiguousThumbnail(String),

    AlreadyPublished,
    CannotMarkPublished,
}

pub type FdahmResult<T> = Result<T, FdahmError>;
