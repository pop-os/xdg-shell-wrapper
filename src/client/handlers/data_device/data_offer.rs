use sctk::data_device_manager::data_offer::DataOfferHandler;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction;

use crate::{space::WrapperSpace, shared_state::GlobalState};

impl<W: WrapperSpace> DataOfferHandler for GlobalState<W> {
    fn offer(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        offer: &mut sctk::data_device_manager::data_offer::DataDeviceOffer,
        mime_type: String,
    ) {}

    // TODO DnD
    fn source_actions(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        offer: &mut sctk::data_device_manager::data_offer::DragOffer,
        actions: DndAction,
    ) {
    }

    fn selected_action(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        offer: &mut sctk::data_device_manager::data_offer::DragOffer,
        actions: DndAction,
    ) {
        todo!()
    }
}
