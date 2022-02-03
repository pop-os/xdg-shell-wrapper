// SPDX-License-Identifier: GPL-3.0-only

use crate::util::*;
use anyhow::Result;
use nix::unistd::fork;
use sctk::reexports::calloop::{self, channel};
use smithay::reexports::wayland_server;
use smithay::wayland::compositor::compositor_init;
use std::os::unix::{io::AsRawFd, net::UnixStream};
use std::process::Command;
use tokio::sync::mpsc::{Receiver, Sender};

pub async fn new_server(_tx: Sender<ServerMsg>, _rx: Receiver<ClientMsg>) -> Result<()> {
    let mut display = wayland_server::Display::new();
    let (display_sock, client_sock) = UnixStream::pair().unwrap();
    display.add_socket_from(display_sock).unwrap();
    let display_client = unsafe { display.create_client(client_sock.as_raw_fd(), &mut ()) };
    compositor_init(&mut display, |surface, dispatch_data| {}, None);

    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        Some(p) => {
            Command::new(p).spawn().expect("command failed to start");
        }
        None => println!("USAGE: xdg_shell_wrapper <executable>"),
    }
    Ok(())
}
