// SPDX-License-Identifier: MPL-2.0-only

use std::{
    os::unix::{net::UnixStream, prelude::AsRawFd},
    process::{Child, Command},
    sync::Arc,
};

use shlex::Shlex;
use slog::{trace, Logger};
use smithay::{
    nix::fcntl,
    reexports::wayland_server::{self, backend::ClientData, Client},
};

/// utility function which maps a value [0, 1] -> [0, 1] using the smootherstep function
pub fn smootherstep(t: f32) -> f32 {
    (6.0 * t.powi(5) - 15.0 * t.powi(4) + 10.0 * t.powi(3)).clamp(0.0, 1.0)
}

/// helper function for inserting a wrapped applet client
pub fn get_client_sock(display: &mut wayland_server::DisplayHandle) -> (Client, UnixStream) {
    let (display_sock, client_sock) = UnixStream::pair().unwrap();
    let raw_fd = client_sock.as_raw_fd();
    let fd_flags =
        fcntl::FdFlag::from_bits(fcntl::fcntl(raw_fd, fcntl::FcntlArg::F_GETFD).unwrap()).unwrap();
    fcntl::fcntl(
        raw_fd,
        fcntl::FcntlArg::F_SETFD(fd_flags.difference(fcntl::FdFlag::FD_CLOEXEC)),
    )
    .unwrap();

    (
        display
            .insert_client(display_sock, Arc::new(WrapperClientData {}))
            .unwrap(),
        client_sock,
    )
}

/// data for wrapper clients
#[derive(Debug)]
pub struct WrapperClientData {}
impl ClientData for WrapperClientData {
    fn initialized(&self, _client_id: wayland_server::backend::ClientId) {}

    fn disconnected(
        &self,
        _client_id: wayland_server::backend::ClientId,
        _reason: wayland_server::backend::DisconnectReason,
    ) {
    }
}

/// helper function for launching a wrapped applet
pub fn exec_child(
    c: &str,
    config_name: Option<&str>,
    log: Logger,
    raw_fd: i32,
    requests_host_wayland_display: bool,
) -> Child {
    let mut exec_iter = Shlex::new(c);
    let exec = exec_iter
        .next()
        .expect("exec parameter must contain at least on word");
    trace!(log, "child: {}", &exec);

    let mut child = Command::new(exec);
    for arg in exec_iter {
        trace!(log, "child argument: {}", &arg);
        child.arg(arg);
    }
    if let Some(config_name) = config_name {
        child.env("COSMIC_DOCK_CONFIG", config_name);
    }

    if requests_host_wayland_display {
        if let Ok(display) = std::env::var("WAYLAND_DISPLAY") {
            child.env("WAYLAND_DISPLAY", display);
        }
    } else {
        child.env_remove("WAYLAND_DISPLAY");
    }

    child
        .env("WAYLAND_SOCKET", raw_fd.to_string())
        .env_remove("WAYLAND_DEBUG")
        // .env("WAYLAND_DEBUG", "1")
        // .stderr(std::process::Stdio::piped())
        // .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start child process")
}
