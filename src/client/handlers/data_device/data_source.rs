use std::os::fd::IntoRawFd;

use crate::{shared_state::GlobalState, space::WrapperSpace};
use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction;
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;

impl<W: WrapperSpace> DataSourceHandler for GlobalState<W> {
    fn send_request(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        fd: sctk::data_device_manager::WritePipe,
    ) {
        let (seat, is_dnd) = match self.server_state.seats.iter().find_map(|seat| {
            seat.client
                .copy_paste_source
                .as_ref()
                .and_then(|sel_source| {
                    if sel_source.inner() == source {
                        Some((seat, false))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    seat.client.dnd_source.as_ref().and_then(|dnd_source| {
                        if dnd_source.inner() == source {
                            Some((seat, true))
                        } else {
                            None
                        }
                    })
                })
        }) {
            Some(seat) => seat,
            None => return,
        };

        // TODO write from server source to fd
        // could be a selection source or a dnd source
        if is_dnd {
            // TODO Dnd
        } else {
            if let Some(selection) = seat.server.selection_source.as_ref() {
                selection.send(mime, fd.into_raw_fd());
            }
        }
   }

    fn accept_mime(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        mime: Option<String>,
    ) {
        // TODO forward the accept mime event
    }

    fn cancelled(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        // TODO forward the cancelled event
    }

    // TODO: DnD
    fn dnd_dropped(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
    }

    fn dnd_finished(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
    }

    fn action(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        action: DndAction,
    ) {
    }
}
