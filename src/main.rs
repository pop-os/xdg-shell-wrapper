// SPDX-License-Identifier: MPL-2.0-only

use anyhow::Result;
use shlex::Shlex;
use slog::{o, trace, Drain};
use std::process::Command;
use xdg_shell_wrapper::{config::Config};
use cosmic_panel_xdg_wrapper::xdg_wrapper;
use cosmic_panel_config::config::XdgWrapperConfig;

fn main() -> Result<()> {
    // A logger facility, here we use the terminal
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        o!(),
    );

    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let arg = std::env::args().nth(1);
    let usage =
        "USAGE: xdg_shell_wrapper '<executable> <app id>' OR xd_shell_wrapper --profile <profile name>";
    let config = match arg.as_ref().map(|s| &s[..]) {
        Some(arg) if arg == "--profile" || arg == "-p" => {
            if let Some(profile) = std::env::args().nth(2) {
                Config::load(profile.as_str())
            } else {
                println!("{}", usage);
                std::process::exit(1);
            }
        }
        Some(exec) => {
            let mut config = Config::default();
            config.exec = exec.into();
            config
        }
        None => {
            println!("{}", usage);
            std::process::exit(1);
        }
    };

    // start child process
    let mut exec_iter = Shlex::new(&config.exec);
    let exec = exec_iter
        .next()
        .expect("exec parameter must contain at least on word");
    trace!(log, "child: {}", &exec);

    let mut child = Command::new(exec);
    while let Some(arg) = exec_iter.next() {
        trace!(log, "child argument: {}", &arg);
        child.arg(arg);
    }
    
    xdg_wrapper(log, config, None)?;
    Ok(())
}
