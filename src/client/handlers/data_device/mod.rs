use sctk::{delegate_data_offer, delegate_data_source, delegate_data_device_manager, delegate_data_device};

use crate::{space::WrapperSpace, shared_state::GlobalState};

pub mod data_offer;
pub mod data_source;
pub mod data_device;

delegate_data_device!(@<W: WrapperSpace+ 'static> GlobalState<W>);
delegate_data_offer!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_data_source!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_data_device_manager!(@<W: WrapperSpace + 'static> GlobalState<W>);

