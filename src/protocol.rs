//! Wire protocol between the CLI client and the daemon.
//!
//! One UTF-8 text line per request; the daemon answers with a single line
//! ("ok..." or "err: ...") except for [`Request::Subscribe`], which holds
//! the connection open and streams one line per update in the requested
//! [`StreamFormat`]. `Request` is the single source of truth: the client
//! serializes via `Display`, the daemon parses via `FromStr`, and the two
//! are round-trip tested so they cannot drift apart.

use crate::error::Error;
use crate::model::{DeskId, Wrap};
use std::fmt;
use std::str::FromStr;

/// Whether moving a window to a desk also switches every monitor there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveMode {
    Follow,
    Silent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Next,
    Prev,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusFormat {
    Text,
    Json,
}

/// What a held-open subscription streams, one line per update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamFormat {
    /// The active desk id.
    DeskId,
    /// A waybar custom-module JSON object (see the `waybar` module).
    Waybar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Switch every monitor to the desk.
    Switch(DeskId),
    /// Move the active window to the desk.
    Move { desk: DeskId, mode: MoveMode },
    /// Step to the neighboring desk.
    Step { direction: Direction, wrap: Wrap },
    /// Back-and-forth to the previously active desk.
    Last,
    /// Report current and last desk.
    Status(StatusFormat),
    /// Stream desk changes until the client disconnects.
    Subscribe(StreamFormat),
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Switch(desk) => write!(f, "vdesk {desk}"),
            Self::Move {
                desk,
                mode: MoveMode::Follow,
            } => write!(f, "movetodesk {desk}"),
            Self::Move {
                desk,
                mode: MoveMode::Silent,
            } => {
                write!(f, "movetodesksilent {desk}")
            }
            Self::Step { direction, wrap } => {
                let name = match direction {
                    Direction::Next => "nextdesk",
                    Direction::Prev => "prevdesk",
                };
                match wrap {
                    Wrap::Clamp => write!(f, "{name}"),
                    Wrap::Cycle => write!(f, "{name} cycle"),
                }
            }
            Self::Last => write!(f, "lastdesk"),
            Self::Status(StatusFormat::Text) => write!(f, "status"),
            Self::Status(StatusFormat::Json) => write!(f, "status json"),
            Self::Subscribe(StreamFormat::DeskId) => write!(f, "subscribe"),
            Self::Subscribe(StreamFormat::Waybar) => write!(f, "waybar"),
        }
    }
}

impl FromStr for Request {
    type Err = Error;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let bad = || Error::BadRequest(line.to_string());
        let mut words = line.split_whitespace();
        let command = words.next().ok_or_else(bad)?;
        let argument = words.next();
        if words.next().is_some() {
            return Err(bad());
        }

        let desk = |arg: Option<&str>| -> Result<DeskId, Error> {
            arg.ok_or_else(bad)?.parse().map_err(|_| bad())
        };
        let wrap = |arg: Option<&str>| -> Result<Wrap, Error> {
            match arg {
                None => Ok(Wrap::Clamp),
                Some("cycle") => Ok(Wrap::Cycle),
                Some(_) => Err(bad()),
            }
        };
        let bare = |req: Self| -> Result<Self, Error> {
            match argument {
                None => Ok(req),
                Some(_) => Err(bad()),
            }
        };

        match command {
            "vdesk" => Ok(Self::Switch(desk(argument)?)),
            "movetodesk" => Ok(Self::Move {
                desk: desk(argument)?,
                mode: MoveMode::Follow,
            }),
            "movetodesksilent" => Ok(Self::Move {
                desk: desk(argument)?,
                mode: MoveMode::Silent,
            }),
            "nextdesk" => Ok(Self::Step {
                direction: Direction::Next,
                wrap: wrap(argument)?,
            }),
            "prevdesk" => Ok(Self::Step {
                direction: Direction::Prev,
                wrap: wrap(argument)?,
            }),
            "lastdesk" => bare(Self::Last),
            "status" => match argument {
                None => Ok(Self::Status(StatusFormat::Text)),
                Some("json") => Ok(Self::Status(StatusFormat::Json)),
                Some(_) => Err(bad()),
            },
            "subscribe" => bare(Self::Subscribe(StreamFormat::DeskId)),
            "waybar" => bare(Self::Subscribe(StreamFormat::Waybar)),
            _ => Err(bad()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk(id: u8) -> DeskId {
        DeskId::new(id).expect("valid desk id in test")
    }

    #[test]
    fn every_request_round_trips() {
        let all = [
            Request::Switch(desk(3)),
            Request::Move {
                desk: desk(10),
                mode: MoveMode::Follow,
            },
            Request::Move {
                desk: desk(1),
                mode: MoveMode::Silent,
            },
            Request::Step {
                direction: Direction::Next,
                wrap: Wrap::Clamp,
            },
            Request::Step {
                direction: Direction::Next,
                wrap: Wrap::Cycle,
            },
            Request::Step {
                direction: Direction::Prev,
                wrap: Wrap::Clamp,
            },
            Request::Step {
                direction: Direction::Prev,
                wrap: Wrap::Cycle,
            },
            Request::Last,
            Request::Status(StatusFormat::Text),
            Request::Status(StatusFormat::Json),
            Request::Subscribe(StreamFormat::DeskId),
            Request::Subscribe(StreamFormat::Waybar),
        ];
        for request in all {
            let line = request.to_string();
            assert_eq!(line.parse::<Request>().ok(), Some(request), "line: {line}");
        }
    }

    #[test]
    fn malformed_lines_are_rejected() {
        for line in [
            "",
            "bogus",
            "vdesk",
            "vdesk 0",
            "vdesk 11",
            "vdesk x",
            "vdesk 3 3",
            "nextdesk sideways",
            "status yaml",
            "lastdesk 2",
            "subscribe now",
            "waybar json",
        ] {
            assert!(line.parse::<Request>().is_err(), "line: {line:?}");
        }
    }
}
