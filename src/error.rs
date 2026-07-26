//! Crate-wide error type. Every fallible path in hyprdesk resolves to one
//! of these variants; `main` renders them for the user and maps them to an
//! exit code.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// A required environment variable is missing; hyprdesk only runs
    /// inside a live Hyprland session.
    MissingEnv(&'static str),
    /// Transport-level failure on a UNIX socket.
    Io(io::Error),
    /// Hyprland answered a JSON query with something we could not parse.
    BadReply {
        query: String,
        source: serde_json::Error,
    },
    /// Hyprland rejected a dispatched command or keyword.
    Rejected(String),
    /// A request line violated the client/daemon wire protocol.
    BadRequest(String),
    /// The daemon's control socket did not answer.
    DaemonUnreachable { path: PathBuf, source: io::Error },
    /// Another daemon already serves this Hyprland instance.
    AlreadyRunning(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv(var) => {
                write!(f, "{var} is not set (not inside a Hyprland session?)")
            }
            Self::Io(source) => write!(f, "socket i/o failed: {source}"),
            Self::BadReply { query, source } => {
                write!(f, "unparsable reply to j/{query}: {source}")
            }
            Self::Rejected(reply) => write!(f, "hyprland rejected a command: {reply}"),
            Self::BadRequest(line) => write!(f, "invalid request: {line}"),
            Self::DaemonUnreachable { path, source } => write!(
                f,
                "cannot reach hyprdesk daemon at {} ({source}); is `hyprdesk daemon` running?",
                path.display()
            ),
            Self::AlreadyRunning(path) => write!(
                f,
                "another hyprdesk daemon is already running on {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) | Self::DaemonUnreachable { source, .. } => Some(source),
            Self::BadReply { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}
