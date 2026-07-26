//! Thin client side of the control socket: send one request line, relay
//! the reply to stdout. For `Subscribe` the reply is an endless stream and
//! this call only returns when the daemon (or the pipe) goes away.

use crate::daemon;
use crate::error::{Error, Result};
use crate::protocol::Request;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

/// Outcome of a served request, for the caller to turn into an exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    /// The daemon answered with an `err: ` reply.
    DaemonError,
}

pub fn send(request: Request) -> Result<Outcome> {
    let path = daemon::control_socket_path()?;
    let mut stream =
        UnixStream::connect(&path).map_err(|source| Error::DaemonUnreachable { path, source })?;
    writeln!(stream, "{request}")?;

    if matches!(request, Request::Subscribe(_)) {
        io::copy(&mut stream, &mut io::stdout())?;
        return Ok(Outcome::Success);
    }

    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    let reply = reply.trim_end();
    println!("{reply}");
    if reply.starts_with("err:") {
        Ok(Outcome::DaemonError)
    } else {
        Ok(Outcome::Success)
    }
}
