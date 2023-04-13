use std::{
    os::fd::{IntoRawFd, OwnedFd},
    rc::Rc,
};

use itertools::Itertools;
use sctk::{
    data_device_manager::data_offer::receive_to_fd,
    reexports::client::{protocol::wl_data_device_manager::DndAction as ClientDndAction, Proxy},
};
use smithay::{
    backend::{egl::{EGLSurface, EGLDisplay}, renderer::{ImportDma, damage::OutputDamageTracker}},
    delegate_data_device, delegate_dmabuf, delegate_output, delegate_primary_selection,
    delegate_seat,
    input::{Seat, SeatHandler, SeatState},
    reexports::wayland_server::{
        protocol::{
            wl_data_device_manager::DndAction, wl_data_source::WlDataSource, wl_surface::WlSurface,
        },
        Resource,
    },
    wayland::{
        data_device::{
            set_data_device_focus, with_source_metadata, ClientDndGrabHandler, DataDeviceHandler,
            ServerDndGrabHandler,
        },
        dmabuf::{DmabufHandler, ImportError},
        primary_selection::{set_primary_focus, PrimarySelectionHandler, PrimarySelectionState},
    }, utils::Transform,
};
use wayland_egl::WlEglSurface;

use crate::{
    shared_state::GlobalState,
    space::{ClientEglSurface, WrapperSpace, ClientEglDisplay},
};

pub(crate) mod compositor;
pub(crate) mod layer;
pub(crate) mod xdg_shell;

impl<W: WrapperSpace> PrimarySelectionHandler for GlobalState<W> {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.server_state.primary_selection_state
    }
}

delegate_primary_selection!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Seat
//

impl<W: WrapperSpace> SeatHandler for GlobalState<W> {
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.server_state.seat_state
    }

    type KeyboardFocus = WlSurface;

    type PointerFocus = WlSurface;

    fn focus_changed(
        &mut self,
        seat: &smithay::input::Seat<Self>,
        focused: Option<&Self::KeyboardFocus>,
    ) {
        let dh = &self.server_state.display_handle;
        if let Some(client) = focused.and_then(|s| dh.get_client(s.id()).ok()) {
            set_data_device_focus(dh, seat, Some(client));
            let client2 = focused.and_then(|s| dh.get_client(s.id()).ok()).unwrap();
            set_primary_focus(dh, seat, Some(client2))
        }
    }

    fn cursor_image(
        &mut self,
        _seat: &smithay::input::Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
        // TODO
    }
}

delegate_seat!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Data Device
//

impl<W: WrapperSpace> DataDeviceHandler for GlobalState<W> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.server_state.data_device_state
    }

    fn new_selection(&mut self, source: Option<WlDataSource>, seat: Seat<GlobalState<W>>) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };

        let serial = seat.client.get_serial_of_last_seat_event();

        if let Some(source) = source {
            seat.client.next_selection_offer_is_mine = true;
            let metadata = with_source_metadata(&source, |metadata| metadata.clone()).unwrap();
            let copy_paste_source = self
                .client_state
                .data_device_manager
                .create_copy_paste_source(
                    &self.client_state.queue_handle,
                    metadata
                        .mime_types
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                );
            seat.client.copy_paste_source = Some(copy_paste_source);
        } else {
            seat.client.data_device.unset_selection(serial)
        }
    }

    fn send_selection(&mut self, mime_type: String, fd: OwnedFd, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };
        if let Some(offer) = seat.client.selection_offer.as_ref() {
            unsafe { receive_to_fd(offer.inner(), mime_type, fd.into_raw_fd()) }
        }
    }
}

impl<W: WrapperSpace> ClientDndGrabHandler for GlobalState<W> {
    fn started(&mut self, source: Option<WlDataSource>, icon: Option<WlSurface>, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };

        if let Some(source) = source.as_ref() {
            seat.client.next_dnd_offer_is_mine = true;
            let metadata = with_source_metadata(&source, |metadata| metadata.clone()).unwrap();
            let mut actions = ClientDndAction::empty();
            if metadata.dnd_action.contains(DndAction::Copy) {
                actions |= ClientDndAction::Copy;
            }
            if metadata.dnd_action.contains(DndAction::Move) {
                actions |= ClientDndAction::Move;
            }
            if metadata.dnd_action.contains(DndAction::Ask) {
                actions |= ClientDndAction::Ask;
            }

            let dnd_source = self
                .client_state
                .data_device_manager
                .create_drag_and_drop_source(
                    &self.client_state.queue_handle,
                    metadata.mime_types.iter().map(|m| m.as_str()).collect_vec(),
                    actions,
                );
            if let Some(focus) = self
                .client_state
                .focused_surface
                .borrow()
                .iter()
                .find(|f| f.1 == seat.name)
            {
                let c_icon_surface = icon.as_ref().map(|_| {
                    self.client_state
                        .compositor_state
                        .create_surface(&self.client_state.queue_handle)
                });
                dnd_source.start_drag(
                    &seat.client.data_device,
                    &focus.0,
                    c_icon_surface.as_ref(),
                    seat.client.get_serial_of_last_seat_event(),
                );
                if let Some(client_surface) = c_icon_surface.as_ref() {
                    client_surface.frame(&self.client_state.queue_handle, client_surface.clone());
                    let renderer = if let Some(r) = self.space.renderer() {
                        r
                    } else {
                        tracing::error!("No renderer available");
                        return;
                    };
                    let client_egl_surface = unsafe {
                        ClientEglSurface::new(
                            WlEglSurface::new(client_surface.id(), 1, 1).unwrap(), // TODO remove unwrap
                            client_surface.clone(),
                        )
                    };
                    let egl_display = EGLDisplay::new(ClientEglDisplay {
                        display: self.client_state.connection.display(),
                    })
                    .expect("Failed to create EGL display");

                    let egl_surface = Rc::new(
                        EGLSurface::new(
                            &egl_display,
                            renderer
                                .egl_context()
                                .pixel_format()
                                .expect("Failed to get pixel format from EGL context "),
                            renderer.egl_context().config_id(),
                            client_egl_surface,
                        )
                        .expect("Failed to create EGL Surface"),
                    );

                    seat.client.dnd_icon = Some((
                        client_surface.clone(),
                        egl_surface,
                        OutputDamageTracker::new(
                            (1, 1),
                            1.0,
                            Transform::Flipped180,
                        ),
                        false,
                        None,
                    ));
                }
            }
            seat.client.dnd_source = Some(dnd_source);
        }
        seat.server.dnd_source = source;
        seat.server.dnd_icon = icon;
    }
}
impl<W: WrapperSpace> ServerDndGrabHandler for GlobalState<W> {
    fn send(&mut self, mime_type: String, fd: OwnedFd, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };
        if let Some(offer) = seat.client.dnd_offer.as_ref() {
            unsafe { receive_to_fd(offer.inner(), mime_type, fd.into_raw_fd()) }
        }
    }

    fn finished(&mut self, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };
        if let Some(offer) = seat.client.dnd_offer.take() {
            offer.finish();
        }
    }

    fn cancelled(&mut self, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };
        if let Some(offer) = seat.client.dnd_offer.take() {
            offer.finish();
        }
    }

    fn action(&mut self, action: DndAction, seat: Seat<Self>) {
        let seat = match self
            .server_state
            .seats
            .iter()
            .find(|s| s.server.seat == seat)
        {
            Some(s) => s,
            None => return,
        };
        let mut c_action = ClientDndAction::empty();
        if action.contains(DndAction::Copy) {
            c_action |= ClientDndAction::Copy;
        }
        if action.contains(DndAction::Move) {
            c_action |= ClientDndAction::Move;
        }
        if action.contains(DndAction::Ask) {
            c_action |= ClientDndAction::Ask;
        }

        if let Some(offer) = seat.client.dnd_offer.as_ref() {
            offer.set_actions(c_action, c_action)
        }
    }
}

delegate_data_device!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Output
//

delegate_output!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Dmabuf
//
impl<W: WrapperSpace> DmabufHandler for GlobalState<W> {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState {
        &mut self.server_state.dmabuf_state.as_mut().unwrap().0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
    ) -> Result<(), ImportError> {
        self.space
            .renderer()
            .map(|renderer| renderer.import_dmabuf(&dmabuf, None))
            .map(|r| match r {
                Ok(_) => Ok(()),
                Err(_) => Err(ImportError::Failed),
            })
            .unwrap_or_else(|| Err(ImportError::Failed))
    }
}
delegate_dmabuf!(@<W: WrapperSpace + 'static> GlobalState<W>);
