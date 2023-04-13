use std::os::fd::IntoRawFd;

use crate::{shared_state::GlobalState, space::WrapperSpace};
use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction as ClientDndAction;
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction;

impl<W: WrapperSpace> DataSourceHandler for GlobalState<W> {
    fn send_request(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
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
            if let Some(dnd_source) = seat.server.dnd_source.as_ref() {
                dnd_source.send(mime, fd.into_raw_fd());
            }
        } else {
            if let Some(selection) = seat.server.selection_source.as_ref() {
                selection.send(mime, fd.into_raw_fd());
            }
        }
    }

    fn accept_mime(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        mime: Option<String>,
    ) {
        let seat = match self.server_state.seats.iter().find(|seat| {
            seat.client
                .dnd_source
                .iter()
                .any(|dnd_source| dnd_source.inner() == source)
        }) {
            Some(seat) => seat,
            None => return,
        };

        if let Some(dnd_source) = seat.server.dnd_source.as_ref() {
            dnd_source.target(mime);
        }
    }

    fn cancelled(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        let seat = match self.server_state.seats.iter_mut().find(|seat| {
            seat.client
                .dnd_source
                .iter()
                .any(|dnd_source| dnd_source.inner() == source)
        }) {
            Some(seat) => seat,
            None => return,
        };

        if let Some(dnd_source) = seat.server.dnd_source.take() {
            dnd_source.cancelled();
            seat.server.dnd_icon = None;
            seat.client.dnd_icon = None;
        }
    }

    // TODO: DnD
    fn dnd_dropped(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        let seat = match self.server_state.seats.iter().find(|seat| {
            seat.client
                .dnd_source
                .iter()
                .any(|dnd_source| dnd_source.inner() == source)
        }) {
            Some(seat) => seat,
            None => return,
        };

        if let Some(dnd_source) = seat.server.dnd_source.as_ref() {
            dnd_source.dnd_drop_performed();
        }
    }

    fn dnd_finished(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        let seat = match self.server_state.seats.iter_mut().find(|seat| {
            seat.client
                .dnd_source
                .iter()
                .any(|dnd_source| dnd_source.inner() == source)
        }) {
            Some(seat) => seat,
            None => return,
        };

        if let Some(dnd_source) = seat.server.dnd_source.take() {
            dnd_source.dnd_finished();
            seat.server.dnd_icon = None;
            seat.client.dnd_icon = None;
        }
    }

    fn action(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        action: ClientDndAction,
    ) {
        let seat = match self.server_state.seats.iter_mut().find(|seat| {
            seat.client
                .dnd_source
                .iter()
                .any(|dnd_source| dnd_source.inner() == source)
        }) {
            Some(seat) => seat,
            None => return,
        };

        let mut dnd_action = DndAction::empty();
        if action.contains(ClientDndAction::Copy) {
            dnd_action |= DndAction::Copy;
        }
        if action.contains(ClientDndAction::Move) {
            dnd_action |= DndAction::Move;
        }
        if action.contains(ClientDndAction::Ask) {
            dnd_action |= DndAction::Ask;
        }

        if let Some(dnd_source) = seat.server.dnd_source.as_ref() {
            dnd_source.action(dnd_action);

        }
 
    }
}
