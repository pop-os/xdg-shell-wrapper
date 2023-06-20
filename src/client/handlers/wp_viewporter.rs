//! Handling of the wp-viewporter.

use std::marker::PhantomData;

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Dispatch;
use sctk::reexports::client::{delegate_dispatch, Connection, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport;
use sctk::reexports::protocols::wp::viewporter::client::wp_viewporter::WpViewporter;

use sctk::globals::GlobalData;

use crate::shared_state::GlobalState;
use crate::space::WrapperSpace;

/// Viewporter.
#[derive(Debug, Clone)]
pub struct ViewporterState<T> {
    viewporter: WpViewporter,
    _phantom: PhantomData<T>,
}

impl<T: 'static + WrapperSpace> ViewporterState<T> {
    /// Create new viewporter.
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<GlobalState<T>>,
    ) -> Result<Self, BindError> {
        let viewporter = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self {
            viewporter,
            _phantom: PhantomData,
        })
    }

    /// Get the viewport for the given object.
    pub fn get_viewport(
        &self,
        surface: &WlSurface,
        queue_handle: &QueueHandle<GlobalState<T>>,
    ) -> WpViewport {
        self.viewporter
            .get_viewport(surface, queue_handle, GlobalData)
    }
}

impl<T: 'static + WrapperSpace> Dispatch<WpViewporter, GlobalData, GlobalState<T>>
    for ViewporterState<T>
{
    fn event(
        _: &mut GlobalState<T>,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<GlobalState<T>>,
    ) {
        // No events.
    }
}

impl<T: 'static + WrapperSpace> Dispatch<WpViewport, GlobalData, GlobalState<T>>
    for ViewporterState<T>
{
    fn event(
        _: &mut GlobalState<T>,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<GlobalState<T>>,
    ) {
        // No events.
    }
}

delegate_dispatch!(@<T: 'static + WrapperSpace> GlobalState<T>: [WpViewporter: GlobalData] => ViewporterState<T>);
delegate_dispatch!(@<T: 'static + WrapperSpace> GlobalState<T>: [WpViewport: GlobalData] => ViewporterState<T>);
