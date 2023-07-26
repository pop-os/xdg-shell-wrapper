use sctk::delegate_data_device;

use crate::{shared_state::GlobalState, space::WrapperSpace};

pub mod data_device;
pub mod data_offer;
pub mod data_source;

delegate_data_device!(@<W: WrapperSpace+ 'static> GlobalState<W>);
