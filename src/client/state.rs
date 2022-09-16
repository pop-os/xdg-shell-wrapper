use std::{cell::RefCell, rc::Rc, time::Instant};

use sctk::{
    compositor::CompositorState,
    output::OutputState,
    reexports::client::{
        protocol::{
            wl_keyboard, wl_pointer,
            wl_seat::WlSeat,
            wl_surface::{self, WlSurface},
        },
        Connection, QueueHandle,
    },
    registry::RegistryState,
    seat::SeatState,
    shell::{layer::LayerState, xdg::XdgShellState},
    shm::{multi::MultiPool, ShmState},
};
use slog::Logger;
use smithay::reexports::calloop;

use crate::{server_state::ServerState, shared_state::GlobalState, space::WrapperSpace};

#[derive(Debug)]
pub(crate) struct ClientSeat {
    pub(crate) _seat: WlSeat,
    pub(crate) kbd: Option<wl_keyboard::WlKeyboard>,
    pub(crate) ptr: Option<wl_pointer::WlPointer>,
}

#[derive(Debug, Copy, Clone)]
/// status of a focus
pub enum FocusStatus {
    /// focused
    Focused,
    /// instant last focused
    LastFocused(Instant),
}
// TODO remove refcell if possible
/// list of focused surfaces and the seats that focus them
pub type ClientFocus = Vec<(wl_surface::WlSurface, String, FocusStatus)>;

/// Wrapper client state
#[derive(Debug)]
pub struct ClientState<W: WrapperSpace + 'static> {
    /// state
    pub registry_state: RegistryState,
    /// state
    pub seat_state: SeatState,
    /// state
    pub output_state: OutputState,
    /// state
    pub compositor_state: CompositorState,
    /// state
    pub shm_state: ShmState,
    /// state
    pub xdg_shell_state: XdgShellState,
    /// state
    pub layer_state: LayerState,

    pub(crate) connection: Connection,
    /// queue handle
    pub queue_handle: QueueHandle<GlobalState<W>>, // TODO remove if never used
    /// state regarding the last embedded client surface with keyboard focus
    pub focused_surface: Rc<RefCell<ClientFocus>>,
    /// state regarding the last embedded client surface with keyboard focus
    pub hovered_surface: Rc<RefCell<ClientFocus>>,
    pub(crate) cursor_surface: Option<wl_surface::WlSurface>,
    pub(crate) multipool: Option<MultiPool<WlSurface>>,
    pub(crate) last_key_pressed: Vec<(String, (u32, u32), wl_surface::WlSurface)>,
}

impl<W: WrapperSpace + 'static> ClientState<W> {
    pub(crate) fn new(
        loop_handle: calloop::LoopHandle<'static, GlobalState<W>>,
        space: &mut W,
        _log: Logger,
        _embedded_server_state: &mut ServerState<W>,
    ) -> anyhow::Result<Self> {
        /*
         * Initial setup
         */
        let connection = Connection::connect_to_env()?;

        let event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let registry_state = RegistryState::new(&connection, &qh);

        let client_state = ClientState {
            focused_surface: space.get_client_focused_surface(),
            hovered_surface: space.get_client_hovered_surface(),

            queue_handle: qh,
            connection,
            seat_state: SeatState::new(),
            output_state: OutputState::new(),
            compositor_state: CompositorState::new(),
            shm_state: ShmState::new(),
            xdg_shell_state: XdgShellState::new(),
            layer_state: LayerState::new(),

            registry_state,
            multipool: None,
            cursor_surface: None,
            last_key_pressed: Vec::new(),
        };

        // TODO refactor to watch outputs and update space when outputs change or new outputs appear
        sctk::event_loop::WaylandSource::new(event_queue)
            .unwrap()
            .insert(loop_handle)
            .unwrap();

        Ok(client_state)
    }
}
