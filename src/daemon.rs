//! The resident daemon: the single owner of desk state.
//!
//! Architecture: two producer threads (the socket2 event pump and the
//! control-socket acceptor) feed one mpsc channel; the main loop consumes
//! it and is the only place state is read or written, so there is no
//! shared-state locking anywhere. Clients get exactly one reply line per
//! request; subscribers are held open and streamed desk changes.
//!
//! Desk state is deliberately minimal - current and last desk. Everything
//! else (monitors, workspaces, windows) is queried live from the
//! compositor at the moment it is needed, so the daemon can never hold a
//! stale copy of the world.

use crate::error::{Error, Result};
use crate::hypr;
use crate::model::{DeskId, monitor_index_of};
use crate::protocol::{Direction, MoveMode, Request, StatusFormat, StreamFormat};
use crate::waybar;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// Control socket path, namespaced by Hyprland instance so a daemon per
/// session is possible and stale sockets from dead sessions never collide.
pub fn control_socket_path() -> Result<PathBuf> {
    let dir = hypr::runtime_dir()?.join("hyprdesk");
    Ok(dir.join(format!("{}.sock", hypr::instance_signature()?)))
}

/// Messages the producer threads feed into the main loop.
enum Message {
    Request(String, UnixStream),
    Subscribe(StreamFormat, UnixStream),
    Event(hypr::Event),
    HyprlandGone,
}

/// A held-open streaming client and the format it consumes.
struct Subscriber {
    format: StreamFormat,
    stream: UnixStream,
}

struct Daemon {
    current: DeskId,
    last: Option<DeskId>,
    subscribers: Vec<Subscriber>,
    /// Last waybar line streamed, to suppress no-op updates from the
    /// chatty window events that feed occupancy.
    waybar_cache: String,
}

pub fn run() -> Result<()> {
    let mut daemon = Daemon {
        current: infer_active_desk().unwrap_or(DeskId::FIRST),
        last: None,
        subscribers: Vec::new(),
        waybar_cache: String::new(),
    };

    // Initial weld: pin rules, repatriate existing workspaces, apply the
    // inferred desk.
    daemon.reweld()?;
    eprintln!("hyprdesk: started on desk {}", daemon.current);

    let listener = bind_control_socket()?;
    let (sender, receiver) = mpsc::channel::<Message>();
    spawn_event_pump(sender.clone());
    spawn_acceptor(listener, sender);

    for message in receiver {
        match message {
            Message::Request(line, mut stream) => {
                let reply = daemon.serve(&line);
                let _ = writeln!(stream, "{reply}");
            }
            Message::Subscribe(format, mut stream) => {
                let greeting = match format {
                    StreamFormat::DeskId => daemon.current.to_string(),
                    StreamFormat::Waybar => daemon.waybar_line(),
                };
                if writeln!(stream, "{greeting}").is_ok() {
                    daemon.subscribers.push(Subscriber { format, stream });
                }
            }
            Message::Event(event) => {
                if let Err(error) = daemon.handle_event(&event) {
                    eprintln!("hyprdesk: handling {event:?} failed: {error}");
                }
            }
            Message::HyprlandGone => {
                eprintln!("hyprdesk: hyprland event socket closed, exiting");
                break;
            }
        }
    }
    let _ = fs::remove_file(control_socket_path()?);
    Ok(())
}

/// Forward socket2 events into the main loop; signal exit when the
/// compositor goes away.
fn spawn_event_pump(sender: mpsc::Sender<Message>) {
    thread::spawn(move || {
        match hypr::event_stream() {
            Ok(events) => {
                for event in events {
                    let message = match event {
                        Ok(event) => Message::Event(event),
                        Err(_) => Message::HyprlandGone,
                    };
                    if sender.send(message).is_err() {
                        return;
                    }
                }
            }
            Err(error) => eprintln!("hyprdesk: cannot open event socket: {error}"),
        }
        let _ = sender.send(Message::HyprlandGone);
    });
}

/// Accept control connections; read one request line each, then hand the
/// stream to the main loop for the reply.
fn spawn_acceptor(listener: UnixListener, sender: mpsc::Sender<Message>) {
    thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(connection) = connection else { continue };
            let sender = sender.clone();
            thread::spawn(move || {
                let mut line = String::new();
                let mut reader = BufReader::new(connection);
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let stream = reader.into_inner();
                let line = line.trim().to_string();
                // Subscriptions are recognized here so the main loop can
                // hold the stream open; anything else (including parse
                // errors) goes down the one-reply request path.
                let message = match line.parse::<Request>() {
                    Ok(Request::Subscribe(format)) => Message::Subscribe(format, stream),
                    _ => Message::Request(line, stream),
                };
                let _ = sender.send(message);
            });
        }
    });
}

fn bind_control_socket() -> Result<UnixListener> {
    let path = control_socket_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if UnixStream::connect(&path).is_ok() {
        return Err(Error::AlreadyRunning(path));
    }
    let _ = fs::remove_file(&path); // stale socket from a dead daemon
    Ok(UnixListener::bind(&path)?)
}

/// The desk shown on the focused monitor, from live compositor state.
fn infer_active_desk() -> Option<DeskId> {
    let active: hypr::WorkspaceRef = hypr::query("activeworkspace").ok()?;
    DeskId::of_workspace(active.id)
}

impl Daemon {
    /// Parse and execute one request line; errors become `err: ` replies
    /// so the client always gets exactly one line.
    fn serve(&mut self, line: &str) -> String {
        line.parse::<Request>()
            .and_then(|request| self.handle_request(request))
            .unwrap_or_else(|error| format!("err: {error}"))
    }

    fn handle_request(&mut self, request: Request) -> Result<String> {
        match request {
            Request::Switch(desk) => self.switch_to(desk),
            Request::Move { desk, mode } => self.move_active_window(desk, mode),
            Request::Step { direction, wrap } => self.switch_to(match direction {
                Direction::Next => self.current.next(wrap),
                Direction::Prev => self.current.prev(wrap),
            }),
            Request::Last => match self.last {
                Some(desk) => self.switch_to(desk),
                None => Ok("ok (no last desk yet)".to_string()),
            },
            Request::Status(format) => Ok(self.status(format)),
            // Subscriptions are intercepted by the acceptor thread;
            // reaching here is a daemon bug, not a client error.
            Request::Subscribe(_) => Err(Error::BadRequest(request.to_string())),
        }
    }

    fn handle_event(&mut self, event: &hypr::Event) -> Result<()> {
        match event {
            hypr::Event::MonitorAdded | hypr::Event::MonitorRemoved => self.reweld(),
            // A config reload wipes runtime workspace rules; re-assert.
            hypr::Event::ConfigReloaded => assert_pinning_rules(&hypr::monitors()?),
            // Track desk changes made behind our back (echoes of our own
            // batches resolve to the desk we already set).
            hypr::Event::WorkspaceChanged => {
                if let Some(desk) = infer_active_desk()
                    && desk != self.current
                {
                    self.record_switch(desk);
                }
                Ok(())
            }
            // Occupancy may have changed; re-render the waybar strip.
            hypr::Event::WindowsChanged => {
                self.refresh_waybar();
                Ok(())
            }
            hypr::Event::Other => Ok(()),
        }
    }

    fn status(&self, format: StatusFormat) -> String {
        let last = self.last.map(|desk| desk.to_string());
        match format {
            StatusFormat::Text => format!(
                "desk: {}\nlast: {}",
                self.current,
                last.unwrap_or_else(|| "none".to_string())
            ),
            StatusFormat::Json => format!(
                "{{\"desk\":{},\"last\":{}}}",
                self.current,
                last.unwrap_or_else(|| "null".to_string())
            ),
        }
    }

    fn record_switch(&mut self, desk: DeskId) {
        self.last = Some(self.current);
        self.current = desk;
        self.notify_subscribers();
    }

    fn notify_subscribers(&mut self) {
        let desk = self.current;
        self.subscribers.retain_mut(|sub| match sub.format {
            StreamFormat::DeskId => writeln!(sub.stream, "{desk}").is_ok(),
            StreamFormat::Waybar => true,
        });
        self.refresh_waybar();
    }

    /// The current waybar strip, rendered from live occupancy.
    fn waybar_line(&self) -> String {
        waybar::status_line(self.current, &occupied_desks().unwrap_or_default())
    }

    /// Re-render the waybar strip and stream it only if it changed.
    fn refresh_waybar(&mut self) {
        let line = self.waybar_line();
        if line == self.waybar_cache {
            return;
        }
        self.subscribers.retain_mut(|sub| match sub.format {
            StreamFormat::Waybar => writeln!(sub.stream, "{line}").is_ok(),
            StreamFormat::DeskId => true,
        });
        self.waybar_cache = line;
    }

    fn switch_to(&mut self, desk: DeskId) -> Result<String> {
        if desk == self.current {
            return Ok("ok (already there)".to_string());
        }
        hypr::batch(&switch_commands(desk, &hypr::monitors()?))?;
        self.record_switch(desk);
        Ok("ok".to_string())
    }

    fn move_active_window(&mut self, desk: DeskId, mode: MoveMode) -> Result<String> {
        let monitors = hypr::monitors()?;
        // The active window lives on the focused monitor and stays there:
        // it lands on that monitor's workspace for the target desk
        // (upstream parity).
        let focused_index = monitors.iter().position(|m| m.focused).unwrap_or(0);
        let target = desk.workspace_on(focused_index);

        match mode {
            MoveMode::Silent => {
                hypr::batch(&[format!("dispatch movetoworkspacesilent {target}")])?;
            }
            MoveMode::Follow => {
                // Switch the other monitors first, then move-and-follow the
                // window last so it ends up focused on its own monitor.
                let mut commands: Vec<String> = monitors
                    .iter()
                    .enumerate()
                    .filter(|(_, monitor)| !monitor.focused)
                    .map(|(index, _)| format!("dispatch workspace {}", desk.workspace_on(index)))
                    .collect();
                commands.push(format!("dispatch movetoworkspace {target}"));
                hypr::batch(&commands)?;
                if desk != self.current {
                    self.record_switch(desk);
                }
            }
        }
        Ok("ok".to_string())
    }

    /// Full re-weld, used at startup and on monitor hotplug: re-pin rules,
    /// repatriate desk workspaces to their owning monitors, merge windows
    /// from workspaces whose monitor slot no longer exists into the desk's
    /// primary workspace (nothing may become unreachable; redock does not
    /// un-merge, per DESIGN), then re-apply the current desk.
    fn reweld(&mut self) -> Result<()> {
        let monitors = hypr::monitors()?;
        if monitors.is_empty() {
            return Ok(());
        }
        assert_pinning_rules(&monitors)?;

        let workspaces: Vec<hypr::Workspace> = hypr::query("workspaces")?;
        let clients: Vec<hypr::Client> = hypr::query("clients")?;
        let mut commands = Vec::new();

        for workspace in &workspaces {
            let (Some(desk), Some(slot)) = (
                DeskId::of_workspace(workspace.id),
                monitor_index_of(workspace.id),
            ) else {
                continue; // special or foreign workspace: not ours to touch
            };
            match monitors.get(slot) {
                None => {
                    // Stranded: its monitor slot is gone.
                    let target = desk.workspace_on(0);
                    commands.extend(
                        clients
                            .iter()
                            .filter(|client| client.workspace.id == workspace.id)
                            .map(|client| {
                                format!(
                                    "dispatch movetoworkspacesilent {target},address:{}",
                                    client.address
                                )
                            }),
                    );
                }
                Some(owner) if workspace.monitor.as_deref() != Some(owner.name.as_str()) => {
                    commands.push(format!(
                        "dispatch moveworkspacetomonitor {} {}",
                        workspace.id, owner.name
                    ));
                }
                Some(_) => {}
            }
        }

        commands.extend(switch_commands(self.current, &monitors));
        hypr::batch(&commands)?;
        self.notify_subscribers();
        Ok(())
    }
}

/// Desks with at least one window on any of their workspaces, across all
/// monitors.
fn occupied_desks() -> Result<Vec<DeskId>> {
    let workspaces: Vec<hypr::Workspace> = hypr::query("workspaces")?;
    let mut occupied: Vec<DeskId> = workspaces
        .iter()
        .filter(|workspace| workspace.windows > 0)
        .filter_map(|workspace| DeskId::of_workspace(workspace.id))
        .collect();
    occupied.sort_unstable();
    occupied.dedup();
    Ok(occupied)
}

/// Commands bringing every monitor to `desk`. The focused monitor is
/// switched last so focus ends where it started (upstream parity).
fn switch_commands(desk: DeskId, monitors: &[hypr::Monitor]) -> Vec<String> {
    let mut commands = Vec::with_capacity(monitors.len());
    let mut focused_command = None;
    for (index, monitor) in monitors.iter().enumerate() {
        let command = format!("dispatch workspace {}", desk.workspace_on(index));
        if monitor.focused {
            focused_command = Some(command);
        } else {
            commands.push(command);
        }
    }
    commands.extend(focused_command);
    commands
}

/// Pin every desk workspace to its monitor with runtime `workspace`
/// keywords, so plain workspace dispatches always land on the right
/// monitor. Config reloads wipe runtime keywords, hence re-assertion on
/// `configreloaded`.
fn assert_pinning_rules(monitors: &[hypr::Monitor]) -> Result<()> {
    let mut commands = Vec::new();
    for (index, monitor) in monitors.iter().enumerate() {
        for desk in DeskId::all() {
            commands.push(format!(
                "keyword workspace {}, monitor:{}",
                desk.workspace_on(index),
                monitor.name
            ));
        }
    }
    hypr::batch(&commands)
}
