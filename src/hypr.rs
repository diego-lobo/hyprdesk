//! Hyprland IPC transport: socket1 (requests) and socket2 (events).
//!
//! This is the only module that talks to the compositor, and it does so
//! exclusively over Hyprland's stable public sockets, never its internal
//! API - that boundary is the reason hyprdesk survives compositor updates.
//!
//! Protocol, verified against Hyprland 0.56.2: connect to `.socket.sock`,
//! write one request, read the reply to EOF. Queries are `j/<name>` and
//! answer JSON; actions are `eval <lua>` chunks answering `ok` (0.56's
//! Lua config engine retired the legacy text grammar: `keyword` answers
//! "can't work with non-legacy parsers" and `dispatch` arguments are now
//! parsed as Lua). `.socket2.sock` streams `event>>data` lines.

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

/// One compositor-side action, in the vocabulary hyprdesk needs. Rendered
/// as an `hl.*` Lua statement and executed through the `eval` request -
/// the only action surface Hyprland 0.56's Lua config engine accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Bring a workspace to its pinned monitor and focus it.
    FocusWorkspace(i64),
    /// Focus a window by compositor address.
    FocusWindow { address: String },
    /// Move a window to a workspace without following it. A vanished
    /// address is a silent no-op, which restore passes rely on.
    MoveWindowSilent { address: String, workspace: i64 },
    /// Move a whole workspace onto a monitor.
    MoveWorkspaceToMonitor { workspace: i64, monitor: String },
    /// Register this workspace's monitor pinning rule. Re-registering a
    /// workspace replaces its previous rule; a config reload wipes all
    /// eval-registered rules (both verified on 0.56.2).
    PinWorkspace { workspace: i64, monitor: String },
}

impl Command {
    /// Render as one Lua statement for the compositor's config engine.
    fn to_lua(&self) -> String {
        match self {
            Self::FocusWorkspace(workspace) => {
                format!("hl.dispatch(hl.dsp.focus({{ workspace = \"{workspace}\" }}))")
            }
            Self::FocusWindow { address } => format!(
                "hl.dispatch(hl.dsp.focus({{ window = {} }}))",
                lua_quote(&format!("address:{address}"))
            ),
            Self::MoveWindowSilent { address, workspace } => format!(
                "hl.dispatch(hl.dsp.window.move({{ window = {}, workspace = \"{workspace}\", follow = false }}))",
                lua_quote(&format!("address:{address}"))
            ),
            Self::MoveWorkspaceToMonitor { workspace, monitor } => format!(
                "hl.dispatch(hl.dsp.workspace.move({{ workspace = \"{workspace}\", monitor = {} }}))",
                lua_quote(monitor)
            ),
            Self::PinWorkspace { workspace, monitor } => format!(
                "hl.workspace_rule({{ workspace = {workspace}, monitor = {} }})",
                lua_quote(monitor)
            ),
        }
    }
}

/// A Lua double-quoted string literal. Compositor-sourced values (monitor
/// names, window addresses) are interpolated into eval chunks; quoting
/// keeps an odd name from being read as Lua syntax.
fn lua_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// Execute commands as one Lua chunk in a single `eval` request. The
/// whole chunk runs inside the compositor with no other input
/// interleaved, tighter than the retired `[[BATCH]]`, which parsed each
/// item separately. A failing statement raises, aborting the rest of the
/// chunk; the reply is then the failure text instead of `ok`.
pub fn eval(commands: &[Command]) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }
    let statements: Vec<String> = commands.iter().map(Command::to_lua).collect();
    let reply = request(&format!("eval {}", statements.join("; ")))?;
    match reply.trim() {
        "ok" => Ok(()),
        rejection => Err(Error::Rejected(rejection.to_string())),
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
    #[serde(rename = "activeWorkspace")]
    pub active_workspace: WorkspaceRef,
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

/// The focused window's address, if any. `activewindow` answers an empty
/// JSON object when no window is focused.
pub fn active_window_address() -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct ActiveWindow {
        address: Option<String>,
    }
    let window: ActiveWindow = query("activewindow")?;
    Ok(window.address)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_render_to_the_verified_lua_forms() {
        assert_eq!(
            Command::FocusWorkspace(15).to_lua(),
            "hl.dispatch(hl.dsp.focus({ workspace = \"15\" }))"
        );
        assert_eq!(
            Command::FocusWindow {
                address: "0xa".to_string()
            }
            .to_lua(),
            "hl.dispatch(hl.dsp.focus({ window = \"address:0xa\" }))"
        );
        assert_eq!(
            Command::MoveWindowSilent {
                address: "0xa".to_string(),
                workspace: 12
            }
            .to_lua(),
            "hl.dispatch(hl.dsp.window.move({ window = \"address:0xa\", \
             workspace = \"12\", follow = false }))"
        );
        assert_eq!(
            Command::MoveWorkspaceToMonitor {
                workspace: 12,
                monitor: "HDMI-A-1".to_string()
            }
            .to_lua(),
            "hl.dispatch(hl.dsp.workspace.move({ workspace = \"12\", monitor = \"HDMI-A-1\" }))"
        );
        assert_eq!(
            Command::PinWorkspace {
                workspace: 21,
                monitor: "eDP-1".to_string()
            }
            .to_lua(),
            "hl.workspace_rule({ workspace = 21, monitor = \"eDP-1\" })"
        );
    }

    #[test]
    fn lua_quote_escapes_metacharacters() {
        assert_eq!(lua_quote("plain"), "\"plain\"");
        assert_eq!(lua_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(lua_quote("a\nb"), "\"a\\nb\"");
    }
}
