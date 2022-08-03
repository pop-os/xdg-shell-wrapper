use crate::{space::WrapperSpace, shared_state::GlobalState};
use sctk::{seat::pointer::PointerHandler, delegate_pointer};

impl<W: WrapperSpace> PointerHandler for GlobalState<W> {
    fn pointer_frame(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        pointer: &sctk::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[sctk::seat::pointer::PointerEvent],
    ) {
        todo!()
    }
}

delegate_pointer!(@<W: WrapperSpace + 'static> GlobalState<W>);
