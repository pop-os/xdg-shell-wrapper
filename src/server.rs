// SPDX-License-Identifier: GPL-3.0-only

use crate::util::*;
use anyhow::Result;
use sctk::reexports::calloop::channel;

pub fn new_server(
    _tx: channel::SyncSender<ServerMsg>,
    _rx: channel::Channel<ClientMsg>,
) -> Result<()> {
    // start application and give
    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        Some(_path) => {
            // TODO start application using anonymous pipe & open fd
        }
        None => println!("USAGE: xdg_shell_wrapper <executable>"),
    }
    Ok(())
}
