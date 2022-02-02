// SPDX-License-Identifier: GPL-3.0-only

use crate::util::*;
use anyhow::Result;
use tokio::sync::mpsc::{Receiver, Sender};

pub async fn new_server(_tx: Sender<ServerMsg>, _rx: Receiver<ClientMsg>) -> Result<()> {
    Ok(())
}
