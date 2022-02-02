// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use tokio::{join, sync::mpsc::channel};
mod client;
mod server;
mod util;

#[tokio::main]
async fn main() -> Result<()> {
    // channel for messages from client to server
    let (client_tx, client_rx) = channel(100);
    // channel for messages from server to client
    let (server_tx, server_rx) = channel(100);

    let (client_res, server_res) = join!(
        client::new_client(client_tx, server_rx),
        server::new_server(server_tx, client_rx)
    );
    client_res?;
    server_res?;
    Ok(())
}
