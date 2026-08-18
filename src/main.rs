//! hyprdesk - shared-monitor virtual desktops for Hyprland.
//!
//! A desk is one logical desktop spanning all connected monitors;
//! switching desks changes what every monitor shows. One binary, two
//! roles: `hyprdesk daemon` runs the resident state owner, every other
//! subcommand is a thin client that sends one request over the control
//! socket (see `protocol`). The compositor is driven exclusively through
//! Hyprland's stable IPC sockets (see `hypr`), never its internal plugin
//! API, so compositor updates cannot break this tool.

mod client;
mod daemon;
mod error;
mod hypr;
mod model;
mod protocol;
mod waybar;

use protocol::{Direction, MoveMode, Request, StatusFormat, StreamFormat};
use std::process::ExitCode;

const USAGE: &str = "\
hyprdesk - shared-monitor virtual desktops for Hyprland

USAGE:
  hyprdesk daemon                start the resident daemon
  hyprdesk vdesk <N>             switch every monitor to desk N (1..=10)
  hyprdesk movetodesk <N>        move active window to desk N and follow
  hyprdesk movetodesksilent <N>  move active window to desk N, stay
  hyprdesk nextdesk [--cycle]    next desk (clamps at 10; --cycle wraps)
  hyprdesk prevdesk [--cycle]    previous desk (clamps at 1; --cycle wraps)
  hyprdesk lastdesk              back-and-forth to the previous desk
  hyprdesk status [--json]       show current/last desk
  hyprdesk subscribe             stream desk changes (one id per line)
  hyprdesk waybar                stream waybar custom-module JSON
                                 (no consumer since Omarchy 4 dropped waybar;
                                 the bar widget derives its own state)";

/// What the command line asks of this process.
enum Invocation {
    RunDaemon,
    Send(Request),
    Usage,
}

fn parse_args(args: &[String]) -> Invocation {
    let mut words = args.iter().map(String::as_str);
    let command = words.next().unwrap_or("");
    let argument = words.next();
    if words.next().is_some() {
        return Invocation::Usage;
    }

    let desk = |arg: Option<&str>| arg.and_then(|a| a.parse().ok());
    let wrap = |arg: Option<&str>| match arg {
        None => Some(model::Wrap::Clamp),
        Some("--cycle") => Some(model::Wrap::Cycle),
        Some(_) => None,
    };

    let request = match (command, argument) {
        ("daemon", None) => return Invocation::RunDaemon,
        ("vdesk", arg) => desk(arg).map(Request::Switch),
        ("movetodesk", arg) => desk(arg).map(|desk| Request::Move {
            desk,
            mode: MoveMode::Follow,
        }),
        ("movetodesksilent", arg) => desk(arg).map(|desk| Request::Move {
            desk,
            mode: MoveMode::Silent,
        }),
        ("nextdesk", arg) => wrap(arg).map(|wrap| Request::Step {
            direction: Direction::Next,
            wrap,
        }),
        ("prevdesk", arg) => wrap(arg).map(|wrap| Request::Step {
            direction: Direction::Prev,
            wrap,
        }),
        ("lastdesk", None) => Some(Request::Last),
        ("status", None) => Some(Request::Status(StatusFormat::Text)),
        ("status", Some("--json")) => Some(Request::Status(StatusFormat::Json)),
        ("subscribe", None) => Some(Request::Subscribe(StreamFormat::DeskId)),
        ("waybar", None) => Some(Request::Subscribe(StreamFormat::Waybar)),
        _ => None,
    };
    request.map_or(Invocation::Usage, Invocation::Send)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Invocation::RunDaemon => match daemon::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("hyprdesk daemon: {error}");
                ExitCode::FAILURE
            }
        },
        Invocation::Send(request) => match client::send(request) {
            Ok(client::Outcome::Success) => ExitCode::SUCCESS,
            Ok(client::Outcome::DaemonError) => ExitCode::FAILURE,
            Err(error) => {
                eprintln!("hyprdesk: {error}");
                ExitCode::FAILURE
            }
        },
        Invocation::Usage => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
