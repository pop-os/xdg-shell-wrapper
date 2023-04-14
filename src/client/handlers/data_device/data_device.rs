use sctk::{
    data_device_manager::{
        data_device::{DataDeviceDataExt, DataDeviceHandler},
        data_offer::{DataOfferData, DataOfferDataExt},
    },
    reexports::client::{
        protocol::wl_data_device_manager::DndAction as ClientDndAction,
        Proxy,
    },
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler},
};
use smithay::{
    input::pointer::GrabStartData,
    reexports::wayland_server::{Resource, protocol::wl_data_device_manager::DndAction},
    utils::SERIAL_COUNTER,
    wayland::data_device::{set_data_device_selection, start_dnd, SourceMetadata, set_data_device_focus},
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

impl<W: WrapperSpace> DataDeviceHandler for GlobalState<W> {
    fn selection(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        // ignore our own selection offer
        if seat.client.next_selection_offer_is_mine {
            seat.client.next_selection_offer_is_mine = false;
            return;
        }

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

        set_data_device_selection(
            &self.server_state.display_handle,
            &seat.server.seat,
            mime_types,
        )
    }

    fn enter(
        &mut self,
        _conn: &sctk::reexports::client::Connection,
        _qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        if seat.client.next_dnd_offer_is_mine {
            seat.client.next_dnd_offer_is_mine = false;
            return;
        }

        let offer = match data_device.drag_offer() {
            Some(offer) => offer,
            None => return,
        };

        let wl_offer = offer.inner();

        let mime_types = wl_offer
            .data::<DataOfferData>()
            .unwrap()
            .data_offer_data()
            .mime_types();
        let mut dnd_action = DndAction::empty();
        let c_action = offer.source_actions;
        if c_action.contains(ClientDndAction::Copy) {
            dnd_action |= DndAction::Copy;
        } else if c_action.contains(ClientDndAction::Move) {
            dnd_action |= DndAction::Move;
        } else if c_action.contains(ClientDndAction::Ask) {
            dnd_action |= DndAction::Ask;
        }

        let metadata = SourceMetadata {
            mime_types,
            dnd_action,
        };

        // Not sure if this will be useful
        // let pointer_event = PointerEvent {
        //     surface: offer.surface,
        //     kind: PointerEventKind::Enter {
        //         serial: offer.serial,
        //     },
        //     position: (offer.x, offer.y),
        // };
        // if let Some(pointer) = pointer.as_ref() {
        //     self.pointer_frame(conn, qh, &pointer, &[pointer_event]);
        // };

        let server_focus = self.space.update_pointer((offer.x as i32, offer.y as i32), &seat.name, offer.surface.clone());
        start_dnd::<_, ()>(
            &self.server_state.display_handle.clone(),
            &seat.server.seat.clone(),
            self,
            SERIAL_COUNTER.next_serial(),
            GrabStartData {
                focus: server_focus.map(|f| (f.surface, f.s_pos)),
                button: 0x110, // assume left button
                location: (offer.x, offer.y).into(), 
            },
            metadata,
        );
    }

    fn leave(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        // TODO stop server DnD?
        // treat it as a pointer leave
        //
        //
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        let offer = match data_device.drag_offer() {
            Some(offer) => offer,
            None => return,
        };
        set_data_device_focus(&self.server_state.display_handle, &seat.server.seat, None);
        let pointer_event = PointerEvent {
            surface: offer.surface,
            kind: PointerEventKind::Leave {
                serial: offer.serial,
            }, // would hope...
            position: (offer.x, offer.y),
        };
        if let Some(pointer) = seat.client.ptr.clone().as_ref() {
            self.pointer_frame(conn, qh, &pointer, &[pointer_event]);
        }
    }

    fn motion(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        // treat it as pointer motion
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        let offer = match data_device.drag_offer() {
            Some(offer) => offer,
            None => return,
        };

        let server_focus = self.space.update_pointer((offer.x as i32, offer.y as i32), &seat.name, offer.surface.clone());
        set_data_device_focus(&self.server_state.display_handle, &seat.server.seat, server_focus.and_then(|f| f.surface.client()));
        let pointer_event = PointerEvent {
            surface: offer.surface,
            kind: PointerEventKind::Motion {
                time: offer.time.unwrap_or_default(),
            },
            position: (offer.x, offer.y),
        };

        if let Some(pointer) = seat.client.ptr.clone().as_ref() {
            self.pointer_frame(conn, qh, &pointer, &[pointer_event]);
        }
    }

    fn drop_performed(
        &mut self,
        conn: &sctk::reexports::client::Connection,
        qh: &sctk::reexports::client::QueueHandle<Self>,
        data_device: sctk::data_device_manager::data_device::DataDevice,
    ) {
        // treat it as pointer button release
        let seat = match self
            .server_state
            .seats
            .iter_mut()
            .find(|sp| sp.client.data_device == data_device)
        {
            Some(sp) => sp,
            None => return,
        };

        let offer = match data_device.drag_offer() {
            Some(offer) => offer,
            None => return,
        };

        let pointer_event = PointerEvent {
            surface: offer.surface,
            kind: PointerEventKind::Release {
                serial: offer.serial,
                time: offer.time.unwrap_or_default(),
                button: 1,
            },
            position: (offer.x, offer.y),
        };
        if let Some(pointer) = seat.client.ptr.clone().as_ref() {
            self.pointer_frame(conn, qh, &pointer, &[pointer_event]);
        }
    }
}
