use sctk::{
    delegate_data_device, delegate_data_device_manager, delegate_data_offer, delegate_data_source,
};

use crate::{shared_state::GlobalState, space::WrapperSpace};

pub mod data_device;
pub mod data_offer;
pub mod data_source;

delegate_data_device!(@<W: WrapperSpace+ 'static> GlobalState<W>);
delegate_data_offer!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_data_source!(@<W: WrapperSpace + 'static> GlobalState<W>);
delegate_data_device_manager!(@<W: WrapperSpace + 'static> GlobalState<W>);
