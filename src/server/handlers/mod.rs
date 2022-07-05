use smithay::{
    delegate_data_device, delegate_output, delegate_seat,
    wayland::{
        data_device::{ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler},
        seat::{SeatHandler, SeatState}, dmabuf::{DmabufHandler, ImportError},
    }, delegate_dmabuf,
    backend::renderer::{ImportDma, gles2::Gles2Error},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

pub(crate) mod compositor;
pub(crate) mod xdg_shell;

//
// Wl Seat
//

impl<W: WrapperSpace> SeatHandler for GlobalState<W> {
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.embedded_server_state.seat_state
    }
}

delegate_seat!(@<W: WrapperSpace + 'static> GlobalState<W>);

//
// Wl Data Device
//

impl<W: WrapperSpace> DataDeviceHandler for GlobalState<W> {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.embedded_server_state.data_device_state
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
        &mut self.embedded_server_state.dmabuf_state.as_mut().unwrap().0
    }

    fn dmabuf_imported(
        &mut self,
        _dh: &smithay::reexports::wayland_server::DisplayHandle,
        _global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
    ) -> Result<(), ImportError> {
            self.space.renderer()
                .map(|renderer| renderer.import_dmabuf(&dmabuf, None))
                .and_then(|r| match r {
                    Ok(_) => Some(Ok(())),
                    Err(e) => Some(Err(ImportError::Failed)),
                })
                .unwrap()
    }
}
delegate_dmabuf!(@<W: WrapperSpace + 'static> GlobalState<W>);
