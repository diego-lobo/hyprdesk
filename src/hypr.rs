//! Hyprland IPC transport: socket1 (requests) and socket2 (events).
//!
//! This is the only module that talks to the compositor, and it does so
//! exclusively over Hyprland's stable public sockets, never its internal
//! API - that boundary is the reason hyprdesk survives compositor updates.
//!
//! Protocol, verified against Hyprland 0.55.4: connect to `.socket.sock`,
//! write one request, read the reply to EOF. Queries are `j/<name>` and
//! answer JSON; dispatches are `dispatch <cmd>` and answer `ok`; batches
//! are `[[BATCH]]a ; b` and answer with the individual replies joined by
//! `\n\n\n`. `.socket2.sock` streams `event>>data` lines.

use crate::error::{Error, Result};
use serde::Deserialize;
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

pub fn runtime_dir() -> Result<PathBuf> {
    env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map_err(|_| Error::MissingEnv("XDG_RUNTIME_DIR"))
}

pub fn instance_signature() -> Result<String> {
    env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| Error::MissingEnv("HYPRLAND_INSTANCE_SIGNATURE"))
}

fn socket_path(name: &str) -> Result<PathBuf> {
    Ok(runtime_dir()?
        .join("hypr")
        .join(instance_signature()?)
        .join(name))
}

/// One request/reply exchange on socket1 (a fresh connection each time,
/// as hyprctl does).
fn request(message: &str) -> Result<String> {
    let mut stream = UnixStream::connect(socket_path(".socket.sock")?)?;
    stream.write_all(message.as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

/// Typed JSON query (`j/<what>`).
pub fn query<T: serde::de::DeserializeOwned>(what: &str) -> Result<T> {
    let reply = request(&format!("j/{what}"))?;
    serde_json::from_str(&reply).map_err(|source| Error::BadReply {
        query: what.to_string(),
        source,
    })
}

/// Run dispatches/keywords as one batch, failing on the first item
/// Hyprland rejects. A batch is processed back-to-back by the compositor;
/// this is as close to an atomic multi-monitor operation as the IPC
/// surface offers.
pub fn batch(commands: &[String]) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }
    let reply = request(&format!("[[BATCH]]{}", commands.join(" ; ")))?;
    match reply
        .split("\n\n\n")
        .map(str::trim)
        .find(|part| !part.is_empty() && *part != "ok")
    {
        Some(rejection) => Err(Error::Rejected(rejection.to_string())),
        None => Ok(()),
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceRef {
    pub id: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub focused: bool,
    pub disabled: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Workspace {
    pub id: i64,
    pub monitor: Option<String>,
    pub windows: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Client {
    pub address: String,
    pub workspace: WorkspaceRef,
}

/// Enabled monitors sorted by Hyprland id. The position in this list is
/// the desk-mapping monitor index (`model::DeskId::workspace_on`); sorting
/// by id keeps the mapping stable across hotplug because the built-in
/// panel enumerates first.
pub fn monitors() -> Result<Vec<Monitor>> {
    let mut monitors: Vec<Monitor> = query("monitors")?;
    monitors.retain(|m| !m.disabled);
    monitors.sort_by_key(|m| m.id);
    Ok(monitors)
}

/// The socket2 events hyprdesk reacts to; everything else is `Other`.
#[derive(Debug)]
pub enum Event {
    MonitorAdded,
    MonitorRemoved,
    ConfigReloaded,
    /// The active workspace changed on some monitor (`workspacev2`).
    WorkspaceChanged,
    /// A window opened, closed, or moved between workspaces - anything
    /// that can change desk occupancy.
    WindowsChanged,
    Other,
}

fn parse_event(line: &str) -> Event {
    let name = line.split_once(">>").map_or(line, |(name, _)| name);
    match name {
        // `monitoradded` and `monitoraddedv2` both fire; react to one only.
        "monitoraddedv2" => Event::MonitorAdded,
        "monitorremoved" => Event::MonitorRemoved,
        "configreloaded" => Event::ConfigReloaded,
        "workspacev2" => Event::WorkspaceChanged,
        // `movewindow` and `movewindowv2` both fire; react to one only.
        "openwindow" | "closewindow" | "movewindowv2" => Event::WindowsChanged,
        _ => Event::Other,
    }
}

/// Subscribe to the socket2 event stream. The iterator ends (with an
/// error item) when Hyprland goes away.
pub fn event_stream() -> Result<impl Iterator<Item = std::io::Result<Event>>> {
    let stream = UnixStream::connect(socket_path(".socket2.sock")?)?;
    Ok(BufReader::new(stream)
        .lines()
        .map(|line| line.map(|l| parse_event(&l))))
}
