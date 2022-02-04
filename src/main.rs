// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use std::thread;
mod client;
mod server;
mod util;
use sctk::reexports::calloop::channel;

fn main() -> Result<()> {
    // A logger facility, here we use the terminal here
    // channel for messages from client to server
    let (client_tx, client_rx) = channel::sync_channel(100);
    // channel for messages from server to client
    let (server_tx, server_rx) = channel::sync_channel(100);

    let client_handle = thread::spawn(move || client::new_client(client_tx, server_rx));
    let server_handle = thread::spawn(move || server::new_server(server_tx, client_rx));
    client_handle.join().unwrap()?;
    server_handle.join().unwrap()?;
    Ok(())
}
