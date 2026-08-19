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
use crate::hypr::{self, Command};
use crate::model::{DeskId, monitor_index_of};
use crate::protocol::{Direction, MoveMode, Request, StatusFormat, StreamFormat};
use crate::waybar;
use std::collections::BTreeMap;
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
    /// Window memory: windows evacuated from a vanished monitor slot,
    /// keyed by window address, holding the home workspace to restore
    /// when that monitor returns. Suspend drops the external monitor's
    /// link, so every sleep looks like an undock; this map is how wake
    /// puts the layout back. Entries live only while the home slot is
    /// missing (see [`restore_commands`]).
    displaced: BTreeMap<String, i64>,
    /// Monitor names as of the last successful re-weld - the daemon's
    /// notion of a settled topology. Hyprland re-juggles workspaces while
    /// a monitor is (un)plugging and emits `workspacev2` BEFORE the
    /// monitor event; those echoes must not be mistaken for the user
    /// switching desks (see the `WorkspaceChanged` arm of
    /// [`Daemon::handle_event`]).
    welded_monitors: Vec<String>,
}

pub fn run() -> Result<()> {
    let mut daemon = Daemon {
        current: infer_active_desk().unwrap_or(DeskId::FIRST),
        last: None,
        subscribers: Vec::new(),
        waybar_cache: String::new(),
        displaced: BTreeMap::new(),
        welded_monitors: Vec::new(),
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
            Request::Status(format) => Ok(render_status(self.current, self.last, format)),
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
            // chunks resolve to the desk we already set). Two guards keep
            // Hyprland's own workspace juggling around a monitor (un)plug
            // from corrupting `current` right before the re-weld re-applies
            // it (seen live: a redock silently reset desk 8 to desk 1):
            // the topology must match the last weld (juggling starts before
            // the monitor event arrives), and every monitor must agree on
            // one desk (juggling and app-driven activation move one monitor
            // at a time; a real desk switch moves them all).
            hypr::Event::WorkspaceChanged => {
                let monitors = hypr::monitors()?;
                if monitor_names(&monitors) == self.welded_monitors
                    && let Some(desk) = desk_shown_everywhere(&monitors)
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
        hypr::eval(&switch_commands(desk, &hypr::monitors()?))?;
        self.record_switch(desk);
        Ok("ok".to_string())
    }

    fn move_active_window(&mut self, desk: DeskId, mode: MoveMode) -> Result<String> {
        let Some(address) = hypr::active_window_address()? else {
            return Ok("ok (no active window)".to_string());
        };
        let monitors = hypr::monitors()?;
        hypr::eval(&move_commands(desk, mode, &monitors, &address))?;
        if mode == MoveMode::Follow && desk != self.current {
            self.record_switch(desk);
        }
        Ok("ok".to_string())
    }

    /// Full re-weld, used at startup and on monitor hotplug: re-pin rules,
    /// repatriate desk workspaces to their owning monitors, evacuate
    /// windows from workspaces whose monitor slot no longer exists into
    /// the desk's primary workspace (nothing may become unreachable),
    /// send previously evacuated windows home if their slot is back
    /// (window memory), then re-apply the current desk.
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
                    // Stranded: its monitor slot is gone. Evacuate each
                    // window to the desk's primary workspace and remember
                    // its home so a returning monitor gets it back.
                    let target = desk.workspace_on(0);
                    for client in clients.iter().filter(|c| c.workspace.id == workspace.id) {
                        self.displaced.insert(client.address.clone(), workspace.id);
                        commands.push(Command::MoveWindowSilent {
                            address: client.address.clone(),
                            workspace: target,
                        });
                    }
                }
                Some(owner) if workspace.monitor.as_deref() != Some(owner.name.as_str()) => {
                    commands.push(Command::MoveWorkspaceToMonitor {
                        workspace: workspace.id,
                        monitor: owner.name.clone(),
                    });
                }
                Some(_) => {}
            }
        }

        commands.extend(restore_commands(
            &mut self.displaced,
            monitors.len(),
            &clients,
        ));
        commands.extend(switch_commands(self.current, &monitors));
        hypr::eval(&commands)?;
        self.welded_monitors = monitor_names(&monitors);
        self.notify_subscribers();
        Ok(())
    }
}

/// Render a status reply. Single-line in both formats: the control
/// protocol is one reply line per request, and a client reading a second
/// line would block. An embedded newline here silently truncated the
/// `last` field for every text-format caller.
fn render_status(current: DeskId, last: Option<DeskId>, format: StatusFormat) -> String {
    match format {
        StatusFormat::Text => {
            let last = last.map_or_else(|| "none".to_string(), |desk| desk.to_string());
            format!("desk: {current}, last: {last}")
        }
        StatusFormat::Json => {
            let last = last.map_or_else(|| "null".to_string(), |desk| desk.to_string());
            format!("{{\"desk\":{current},\"last\":{last}}}")
        }
    }
}

/// The monitor names in slot order - the daemon's notion of "topology"
/// for the settled-topology guard on workspace events.
fn monitor_names(monitors: &[hypr::Monitor]) -> Vec<String> {
    monitors.iter().map(|m| m.name.clone()).collect()
}

/// The desk every monitor is showing, if they agree: monitor slot `m`
/// must show that desk's workspace for `m`. States where monitors show
/// pieces of different desks (mid-hotplug juggling, app-driven activation
/// switching one monitor) resolve to `None` - a desk switch is only
/// tracked when the world looks like exactly one desk.
fn desk_shown_everywhere(monitors: &[hypr::Monitor]) -> Option<DeskId> {
    let desk = DeskId::of_workspace(monitors.first()?.active_workspace.id)?;
    monitors
        .iter()
        .enumerate()
        .all(|(index, monitor)| monitor.active_workspace.id == desk.workspace_on(index))
        .then_some(desk)
}

/// Commands moving the window at `address` to `desk`, following it there
/// or not per `mode`. The window lands on the focused monitor's workspace
/// for the target desk, staying on its own monitor (upstream parity).
///
/// The window is pinned by address throughout: a dispatch that relies on
/// "the active window" resolves it at execution time, mid-chunk, where a
/// preceding cross-monitor focus dispatch has already moved focus - the
/// unpinned form then moves whatever window focus landed on (seen live
/// after a redock: the stale focus pointer moved the wrong window). A
/// follow ends with an explicit window focus for the same reason.
fn move_commands(
    desk: DeskId,
    mode: MoveMode,
    monitors: &[hypr::Monitor],
    address: &str,
) -> Vec<Command> {
    let focused_index = monitors.iter().position(|m| m.focused).unwrap_or(0);
    let mut commands = vec![Command::MoveWindowSilent {
        address: address.to_string(),
        workspace: desk.workspace_on(focused_index),
    }];
    if mode == MoveMode::Follow {
        commands.extend(switch_commands(desk, monitors));
        commands.push(Command::FocusWindow {
            address: address.to_string(),
        });
    }
    commands
}

/// Commands sending displaced windows home, run on every re-weld. An
/// entry survives only while its home monitor slot is still missing; the
/// moment the slot is back the entry's fate is settled in one pass: the
/// window is sent home if it still sits where evacuation left it, and
/// forgotten otherwise (closed, or deliberately re-placed by the user
/// while displaced - a placement we must respect, not undo).
fn restore_commands(
    displaced: &mut BTreeMap<String, i64>,
    monitor_count: usize,
    clients: &[hypr::Client],
) -> Vec<Command> {
    let mut commands = Vec::new();
    displaced.retain(|address, home| {
        let (Some(desk), Some(slot)) = (DeskId::of_workspace(*home), monitor_index_of(*home))
        else {
            return false; // impossible by construction; drop defensively
        };
        if slot >= monitor_count {
            return true; // home monitor still absent: keep waiting
        }
        let still_where_evacuated = clients.iter().any(|client| {
            client.address == *address && client.workspace.id == desk.workspace_on(0)
        });
        if still_where_evacuated {
            commands.push(Command::MoveWindowSilent {
                address: address.clone(),
                workspace: *home,
            });
        }
        false
    });
    commands
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
fn switch_commands(desk: DeskId, monitors: &[hypr::Monitor]) -> Vec<Command> {
    let mut commands = Vec::with_capacity(monitors.len());
    let mut focused_command = None;
    for (index, monitor) in monitors.iter().enumerate() {
        let command = Command::FocusWorkspace(desk.workspace_on(index));
        if monitor.focused {
            focused_command = Some(command);
        } else {
            commands.push(command);
        }
    }
    commands.extend(focused_command);
    commands
}

/// Pin every desk workspace to its monitor with eval-registered workspace
/// rules, so plain workspace focus dispatches always land on the right
/// monitor. Config reloads wipe eval-registered rules, hence re-assertion
/// on `configreloaded`.
fn assert_pinning_rules(monitors: &[hypr::Monitor]) -> Result<()> {
    let mut commands = Vec::new();
    for (index, monitor) in monitors.iter().enumerate() {
        for desk in DeskId::all() {
            commands.push(Command::PinWorkspace {
                workspace: desk.workspace_on(index),
                monitor: monitor.name.clone(),
            });
        }
    }
    hypr::eval(&commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(address: &str, workspace: i64) -> hypr::Client {
        hypr::Client {
            address: address.to_string(),
            workspace: hypr::WorkspaceRef { id: workspace },
        }
    }

    fn displaced(entries: &[(&str, i64)]) -> BTreeMap<String, i64> {
        entries
            .iter()
            .map(|(address, home)| ((*address).to_string(), *home))
            .collect()
    }

    fn monitor(id: i64, name: &str, focused: bool, active_workspace: i64) -> hypr::Monitor {
        hypr::Monitor {
            id,
            name: name.to_string(),
            focused,
            disabled: false,
            active_workspace: hypr::WorkspaceRef {
                id: active_workspace,
            },
        }
    }

    fn desk(id: u8) -> DeskId {
        DeskId::new(id).expect("valid desk id in test")
    }

    fn move_silent(address: &str, workspace: i64) -> Command {
        Command::MoveWindowSilent {
            address: address.to_string(),
            workspace,
        }
    }

    #[test]
    fn status_replies_fit_on_one_line_in_both_formats() {
        // One reply line per request is the control-protocol invariant;
        // a second line would be left unread in the client's socket.
        for format in [StatusFormat::Text, StatusFormat::Json] {
            for last in [None, Some(desk(1))] {
                let reply = render_status(desk(5), last, format);
                assert!(!reply.contains('\n'), "multi-line reply: {reply}");
            }
        }
    }

    #[test]
    fn status_reports_the_current_and_last_desk() {
        assert_eq!(
            render_status(desk(5), Some(desk(2)), StatusFormat::Text),
            "desk: 5, last: 2"
        );
        assert_eq!(
            render_status(desk(5), None, StatusFormat::Text),
            "desk: 5, last: none"
        );
        assert_eq!(
            render_status(desk(5), Some(desk(2)), StatusFormat::Json),
            r#"{"desk":5,"last":2}"#
        );
        assert_eq!(
            render_status(desk(5), None, StatusFormat::Json),
            r#"{"desk":5,"last":null}"#
        );
    }

    #[test]
    fn window_waits_while_its_home_monitor_is_missing() {
        // Home ws 12 lives on monitor slot 1; only one monitor is present.
        let mut memory = displaced(&[("0xa", 12)]);
        let commands = restore_commands(&mut memory, 1, &[client("0xa", 2)]);
        assert!(commands.is_empty());
        assert_eq!(memory, displaced(&[("0xa", 12)]));
    }

    #[test]
    fn window_goes_home_when_its_monitor_returns() {
        // Evacuation left it on ws 2 (desk 2's primary); slot 1 is back.
        let mut memory = displaced(&[("0xa", 12)]);
        let commands = restore_commands(&mut memory, 2, &[client("0xa", 2)]);
        assert_eq!(commands, vec![move_silent("0xa", 12)]);
        assert!(memory.is_empty());
    }

    #[test]
    fn closed_or_replaced_windows_are_left_alone_and_forgotten() {
        // 0xa was re-placed onto another desk while displaced; 0xb closed.
        let mut memory = displaced(&[("0xa", 12), ("0xb", 15)]);
        let commands = restore_commands(&mut memory, 2, &[client("0xa", 7)]);
        assert!(commands.is_empty());
        assert!(memory.is_empty());
    }

    #[test]
    fn follow_move_pins_the_window_and_ends_focused_on_it() {
        // Focused on the external (slot 1): the window lands on desk 5's
        // external workspace; the switch runs the focused monitor last;
        // focus is pinned back to the moved window, immune to the focus
        // steal from the cross-monitor workspace dispatch.
        let monitors = [monitor(0, "eDP-1", false, 1), monitor(1, "DP-2", true, 11)];
        let commands = move_commands(desk(5), MoveMode::Follow, &monitors, "0xa");
        assert_eq!(
            commands,
            vec![
                move_silent("0xa", 15),
                Command::FocusWorkspace(5),
                Command::FocusWorkspace(15),
                Command::FocusWindow {
                    address: "0xa".to_string()
                },
            ]
        );
    }

    #[test]
    fn silent_move_pins_the_window_and_switches_nothing() {
        let monitors = [monitor(0, "eDP-1", true, 1), monitor(1, "DP-2", false, 11)];
        let commands = move_commands(desk(5), MoveMode::Silent, &monitors, "0xa");
        assert_eq!(commands, vec![move_silent("0xa", 5)]);
    }

    #[test]
    fn single_monitor_follow_move_still_switches_and_refocuses() {
        let monitors = [monitor(0, "eDP-1", true, 1)];
        let commands = move_commands(desk(3), MoveMode::Follow, &monitors, "0xb");
        assert_eq!(
            commands,
            vec![
                move_silent("0xb", 3),
                Command::FocusWorkspace(3),
                Command::FocusWindow {
                    address: "0xb".to_string()
                },
            ]
        );
    }

    #[test]
    fn a_desk_shown_on_every_monitor_is_recognized() {
        let monitors = [monitor(0, "eDP-1", true, 2), monitor(1, "DP-2", false, 12)];
        assert_eq!(desk_shown_everywhere(&monitors), Some(desk(2)));
        assert_eq!(
            desk_shown_everywhere(&[monitor(0, "eDP-1", true, 5)]),
            Some(desk(5))
        );
    }

    #[test]
    fn mixed_desk_states_are_not_a_switch() {
        // Mid-hotplug juggling: one monitor moved, the other not yet.
        let mixed = [monitor(0, "eDP-1", true, 8), monitor(1, "DP-2", false, 12)];
        assert_eq!(desk_shown_everywhere(&mixed), None);
        // A workspace shown on the wrong slot is not a desk state either.
        let misplaced = [monitor(0, "eDP-1", true, 12)];
        assert_eq!(desk_shown_everywhere(&misplaced), None);
        // Special workspaces and an empty monitor list resolve to nothing.
        let special = [monitor(0, "eDP-1", true, -98)];
        assert_eq!(desk_shown_everywhere(&special), None);
        assert_eq!(desk_shown_everywhere(&[]), None);
    }

    #[test]
    fn restores_are_deterministic_and_batched_together() {
        let mut memory = displaced(&[("0xb", 15), ("0xa", 12)]);
        let commands = restore_commands(&mut memory, 2, &[client("0xa", 2), client("0xb", 5)]);
        assert_eq!(
            commands,
            vec![move_silent("0xa", 12), move_silent("0xb", 15)]
        );
        assert!(memory.is_empty());
    }
}
