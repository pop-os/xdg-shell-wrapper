use sctk::{
    data_device_manager::{
        data_device::{DataDeviceDataExt, DataDeviceHandler},
        data_offer::{DataOfferData, DataOfferDataExt},
    },
    reexports::client::Proxy,
};
use smithay::wayland::primary_selection::set_primary_selection;

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> DataDeviceHandler for GlobalState<W> {
    fn selection(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        let seat = match self
            .server_state
            .seats
            .iter()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        let offer = match data_device.selection_offer() {
            Some(offer) => offer,
            None => return,
        };
        let wl_offer = offer.inner();

        let mime_types = wl_offer
            .data::<DataOfferData>()
            .unwrap()
            .data_offer_data()
            .mime_types();

        set_primary_selection(&self.server_state.display_handle, &seat.server, mime_types)
    }

    // TODO
    fn enter(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
    }

    fn leave(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
    }

    fn motion(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
    }

    fn drop_performed(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
    }
}
