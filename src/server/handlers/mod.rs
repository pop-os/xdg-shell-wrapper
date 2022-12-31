use smithay::{
    backend::renderer::ImportDma,
    delegate_data_device, delegate_dmabuf, delegate_output, delegate_primary_selection,
    delegate_seat,
    input::{SeatHandler, SeatState},
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Resource},
    wayland::{
        data_device::{
            set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
        },
        dmabuf::{DmabufHandler, ImportError},
        primary_selection::{set_primary_focus, PrimarySelectionHandler, PrimarySelectionState},
    },
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

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
}

impl<W: WrapperSpace> ClientDndGrabHandler for GlobalState<W> {}
impl<W: WrapperSpace> ServerDndGrabHandler for GlobalState<W> {}

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
