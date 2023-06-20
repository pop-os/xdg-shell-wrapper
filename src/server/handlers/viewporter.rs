use smithay::delegate_viewporter;

use crate::{shared_state::GlobalState, space::WrapperSpace};

delegate_viewporter!(@<W: WrapperSpace + 'static> GlobalState<W>);
