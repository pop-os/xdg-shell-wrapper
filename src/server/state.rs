use slog::Logger;
use smithay::{
    desktop::PopupManager,
    input::{Seat, SeatState},
    reexports::wayland_server::{protocol::wl_surface::WlSurface, DisplayHandle},
    utils::{Logical, Point},
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        dmabuf::{DmabufGlobal, DmabufState},
        output::OutputManagerState,
        primary_selection::PrimarySelectionState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
    },
};

use crate::{client_state::ClientSeat, shared_state::GlobalState, space::WrapperSpace};

/// list of focused surfaces and the seats that focus them

pub type ServerFocus = Vec<(WlSurface, String)>;
#[allow(missing_debug_implementations)]

/// Information for tracking the server pointer focus
#[derive(Debug, Clone)]
pub struct ServerPointerFocus {
    /// focused wl surface
    pub surface: WlSurface,
    /// name of the seat which is focusing
    pub seat_name: String,
    /// location in compositor space for the layer shell surface or popup
    pub c_pos: Point<i32, Logical>,
    /// location of the focused embedded surface in compositor space
    pub s_pos: Point<i32, Logical>,
}

/// helper type for focus
pub type ServerPtrFocus = Vec<ServerPointerFocus>;

#[allow(missing_debug_implementations)]
/// internal server state
pub struct ServerState<W: WrapperSpace + 'static> {
    /// popup manager
    pub popup_manager: PopupManager,
    pub(crate) display_handle: DisplayHandle,
    // pub(crate) selected_data_provider: SelectedDataProvider,
    pub(crate) last_button: Option<u32>,
    pub(crate) seats: Vec<SeatPair<W>>,
    // Smithay State
    pub(crate) compositor_state: CompositorState,
    pub(crate) xdg_shell_state: XdgShellState,
    pub(crate) shm_state: ShmState,
    pub(crate) _output_manager_state: OutputManagerState,
    pub(crate) seat_state: SeatState<GlobalState<W>>,
    pub(crate) data_device_state: DataDeviceState,
    pub(crate) dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
    pub(crate) primary_selection_state: PrimarySelectionState,
    pub(crate) layer_shell_state: WlrLayerShellState,
}

impl<W: WrapperSpace> ServerState<W> {
    pub(crate) fn new(dh: DisplayHandle, log: Logger) -> ServerState<W> {
        ServerState {
            popup_manager: PopupManager::new(log.clone()),
            display_handle: dh.clone(),
            last_button: None,
            seats: Vec::new(),
            compositor_state: CompositorState::new::<GlobalState<W>, _>(&dh, log.clone()),
            xdg_shell_state: XdgShellState::new::<GlobalState<W>, _>(&dh, log.clone()),
            shm_state: ShmState::new::<GlobalState<W>, _>(&dh, vec![], log.clone()),
            _output_manager_state: OutputManagerState::new_with_xdg_output::<GlobalState<W>>(&dh),
            seat_state: SeatState::new(),
            data_device_state: DataDeviceState::new::<GlobalState<W>, _>(&dh, log.clone()),
            primary_selection_state: PrimarySelectionState::new::<GlobalState<W>, _>(
                &dh,
                log.clone(),
            ),
            layer_shell_state: WlrLayerShellState::new::<GlobalState<W>, _>(&dh, log.clone()),
            dmabuf_state: None,
        }
    }
}

pub(crate) struct SeatPair<W: WrapperSpace + 'static> {
    pub(crate) name: String,
    pub(crate) client: ClientSeat,
    pub(crate) server: Seat<GlobalState<W>>,
}
