use cctk::workspace::WorkspaceHandler;

use crate::{
    shared_state::GlobalState,
    space::{WorkspaceHandlerSpace, WrapperSpace},
};

impl<W: WrapperSpace + WorkspaceHandlerSpace> WorkspaceHandler for GlobalState<W> {
    fn workspace_state(&mut self) -> &mut cctk::workspace::WorkspaceState {
        self.client_state.workspace_state.as_mut().unwrap()
    }

    fn done(&mut self) {
        let groups = self
            .client_state
            .workspace_state
            .as_ref()
            .unwrap()
            .workspace_groups();
        WorkspaceHandlerSpace::update(&mut self.space, groups);
    }
}

cctk::delegate_workspace!(@<W: WrapperSpace + WorkspaceHandlerSpace + 'static> GlobalState<W>);
