// From: https://github.com/rust-windowing/winit/blob/master/src/platform_impl/linux/wayland/types/wp_fractional_scaling.rs
//! Handling of the fractional scaling.

use std::marker::PhantomData;

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Dispatch;
use sctk::reexports::client::{delegate_dispatch, Connection, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use sctk::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::Event as FractionalScalingEvent;
use sctk::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;

use sctk::globals::GlobalData;

use crate::shared_state::GlobalState;
use crate::space::WrapperSpace;

/// The scaling factor denominator.
const SCALE_DENOMINATOR: f64 = 120.;

/// Fractional scaling manager.
#[derive(Debug, Clone)]
pub struct FractionalScalingManager<T> {
    manager: WpFractionalScaleManagerV1,

    _phantom: PhantomData<T>,
}

/// Fractional scaling data.
#[derive(Debug, Clone)]
pub struct FractionalScaling {
    /// The surface used for scaling.
    surface: WlSurface,
}

impl<T: 'static + WrapperSpace> FractionalScalingManager<T> {
    /// Create new viewporter.
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<GlobalState<T>>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self {
            manager,
            _phantom: PhantomData,
        })
    }

    /// Create a fractional scaling object for a given surface.
    pub fn fractional_scaling(
        &self,
        surface: &WlSurface,
        queue_handle: &QueueHandle<GlobalState<T>>,
    ) -> WpFractionalScaleV1 {
        let data = FractionalScaling {
            surface: surface.clone(),
        };
        self.manager
            .get_fractional_scale(surface, queue_handle, data)
    }
}

impl<T: 'static + WrapperSpace> Dispatch<WpFractionalScaleManagerV1, GlobalData, GlobalState<T>>
    for FractionalScalingManager<T>
{
    fn event(
        _: &mut GlobalState<T>,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<GlobalState<T>>,
    ) {
        // No events.
    }
}

impl<T: 'static + WrapperSpace> Dispatch<WpFractionalScaleV1, FractionalScaling, GlobalState<T>>
    for FractionalScalingManager<T>
{
    fn event(
        state: &mut GlobalState<T>,
        _: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &FractionalScaling,
        _: &Connection,
        _: &QueueHandle<GlobalState<T>>,
    ) {
        if let FractionalScalingEvent::PreferredScale { scale } = event {
            state.scale_factor_changed(&data.surface, scale as f64 / SCALE_DENOMINATOR, false);
        }
    }
}

delegate_dispatch!(@<T: 'static + WrapperSpace> GlobalState<T>: [WpFractionalScaleManagerV1: GlobalData] => FractionalScalingManager<T>);
delegate_dispatch!(@<T: 'static + WrapperSpace> GlobalState<T>: [WpFractionalScaleV1: FractionalScaling] => FractionalScalingManager<T>);
