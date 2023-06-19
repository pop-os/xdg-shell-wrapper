use smithay::delegate_viewporter;

use crate::{space::WrapperSpace, shared_state::GlobalState};

delegate_viewporter!(@<W: WrapperSpace + 'static> GlobalState<W>);
