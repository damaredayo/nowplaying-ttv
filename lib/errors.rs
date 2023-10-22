use core::fmt;

pub type NPResult<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    ConfigError,
    FileError,
    SoundcloudError,
    SpotifyError,
    TwitchError,
    HttpError,
    HyperError,
    ParseError,
    Restarting,
    UnknownError,
}

impl Error {
    pub fn new(message: String, kind: ErrorKind) -> Self {
        Self { message, kind }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ErrorKind::ConfigError => "ConfigError",
            ErrorKind::FileError => "FileError",
            ErrorKind::SoundcloudError => "SoundcloudError",
            ErrorKind::SpotifyError => "SpotifyError",
            ErrorKind::TwitchError => "TwitchError",
            ErrorKind::HttpError => "HttpError",
            ErrorKind::HyperError => "HyperError",
            ErrorKind::ParseError => "ParseError",
            ErrorKind::Restarting => "Restarting",
            ErrorKind::UnknownError => "UnknownError",
        };

        write!(f, "{}: {}", kind, self.message)
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        &self.message
    }
}

impl std::convert::From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ErrorKind::HyperError,
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            message: e.to_string(),
            kind: ErrorKind::UnknownError,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ErrorKind::FileError,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ErrorKind::HttpError,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ErrorKind::ParseError,
        }
    }
}
