// SPDX-License-Identifier: MPL-2.0-only

use std::{
    io::{BufWriter, Write},
    os::unix::{net::UnixStream, prelude::AsRawFd},
    process::{Child, Command},
    ptr::write,
    sync::Arc,
};

use shlex::Shlex;
use slog::{trace, Logger};
use smithay::{
    nix::fcntl,
    reexports::wayland_server::{self, backend::ClientData, Client},
};
// SPDX-License-Identifier: MPL-2.0-only
use anyhow::{bail, Result};
use sctk::{
    reexports::client::{
        protocol::{wl_shm, wl_surface::WlSurface},
        QueueHandle,
    },
    shm::multi::MultiPool,
};
use smithay::{
    backend::renderer::{buffer_type, BufferType},
    wayland::{
        compositor::BufferAssignment,
        shm::{with_buffer_contents, BufferData},
    },
};

use crate::shared_state::GlobalState;

use super::WrapperSpace;

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
    log: Logger,
    raw_fd: i32,
    env_vars: Vec<(&str, &str)>,
    requests_wayland_display: bool,
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

    for (key, val) in &env_vars {
        child.env(key, val);
    }

    if !requests_wayland_display {
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

pub(crate) fn write_and_attach_buffer<W: WrapperSpace + 'static>(
    buffer_assignment: &BufferAssignment,
    cursor_surface: &WlSurface,
    multipool: &mut MultiPool<WlSurface>,
    qh: &QueueHandle<GlobalState<W>>,
) -> Result<()> {
    if let BufferAssignment::NewBuffer(source_buffer) = buffer_assignment {
        if let Some(BufferType::Shm) = buffer_type(source_buffer) {
            with_buffer_contents(
                source_buffer,
                move |from: &[u8], buffer_metadata: BufferData| {
                    if let Ok(format) = wl_shm::Format::try_from(buffer_metadata.format as u32) {
                        let BufferData {
                            offset,
                            width,
                            height,
                            stride,
                            ..
                        } = buffer_metadata;
                        let (_, buff, to) =
                            match multipool.get(width, stride, height, cursor_surface, format) {
                                Some(b) => b,
                                None => {
                                    if let Ok(b) = multipool.create_buffer(
                                        width,
                                        stride,
                                        height,
                                        cursor_surface,
                                        format,
                                    ) {
                                        b
                                    } else {
                                        // try again
                                        match multipool.create_buffer(
                                            width,
                                            stride,
                                            height,
                                            cursor_surface,
                                            format,
                                        ) {
                                            Ok(b) => b,
                                            Err(e) => bail!("Failed to create buffer {}", e),
                                        }
                                    }
                                }
                            };

                        let mut writer = BufWriter::new(to);
                        let from = Vec::from(from);
                        let offset: usize = offset.try_into()?;
                        let width: usize = width.try_into()?;
                        let height: usize = height.try_into()?;
                        let stride: usize = stride.try_into()?;

                        writer.write_all(&from[offset..(offset + width * height * stride)])?;
                        writer.flush()?;

                        cursor_surface.attach(Some(buff), 0, 0);
                        cursor_surface.commit();
                        Ok(())
                    } else {
                        bail!("unsupported format!")
                    }
                },
            )?
        } else {
            bail!("not an shm buffer...")
        }
    } else {
        Ok(())
    }
}
