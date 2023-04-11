use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction;
use crate::{space::WrapperSpace, shared_state::GlobalState};



impl<W: WrapperSpace> DataSourceHandler for GlobalState<W> {
    fn send_request(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        fd: sctk::data_device_manager::WritePipe,
    ) {
    }

    fn accept_mime(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        source: &WlDataSource,
        mime: Option<String>,
    ) {
        todo!()
    }


    fn cancelled(&mut self, conn: &sctk::reexports::client::Connection, qh: &sctk::reexports::client::QueueHandle<Self>, source: &WlDataSource) {
        todo!()
    }

    // TODO: DnD
    fn dnd_dropped(&mut self, conn: &sctk::reexports::client::Connection, qh: &sctk::reexports::client::QueueHandle<Self>, source: &WlDataSource) {
    }

    fn dnd_finished(&mut self, conn: &sctk::reexports::client::Connection, qh: &sctk::reexports::client::QueueHandle<Self>, source: &WlDataSource) {
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
