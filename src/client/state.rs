use std::{cell::RefCell, rc::Rc, time::Instant};

use sctk::{
    compositor::CompositorState,
    output::OutputState,
    reexports::client::{
        protocol::{
            wl_keyboard, wl_pointer,
            wl_seat::WlSeat,
            wl_shm,
            wl_surface::{self, WlSurface},
        },
        Connection, QueueHandle,
    },
    registry::RegistryState,
    seat::SeatState,
    shell::xdg::XdgShellState,
    shm::{multi::MultiPool, ShmState},
};
use slog::Logger;
use smithay::reexports::{calloop, wayland_server};

use crate::{
    server_state::ServerState,
    shared_state::{GlobalState},
    space::WrapperSpace,
};

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
    pub(crate) registry_state: RegistryState,
    pub(crate) seat_state: SeatState,
    pub(crate) output_state: OutputState,
    pub(crate) compositor_state: CompositorState,
    pub(crate) shm_state: ShmState,
    pub(crate) xdg_shell_state: XdgShellState,

    pub(crate) connection: Connection,
    pub(crate) _queue_handle: QueueHandle<GlobalState<W>>, // TODO remove if never used
    /// state regarding the last embedded client surface with keyboard focus
    pub focused_surface: Rc<RefCell<ClientFocus>>,
    /// state regarding the last embedded client surface with keyboard focus
    pub hovered_surface: Rc<RefCell<ClientFocus>>,
    pub(crate) cursor_surface: Option<wl_surface::WlSurface>,
    pub(crate) shm: Option<wl_shm::WlShm>,
    pub(crate) multipool: Option<MultiPool<WlSurface>>,
}

impl<W: WrapperSpace + 'static> ClientState<W> {
    pub(crate) fn new(
        loop_handle: calloop::LoopHandle<'static, GlobalState<W>>,
        space: &mut W,
        _log: Logger,
        dh: &mut wayland_server::DisplayHandle,
        _embedded_server_state: &mut ServerState<W>,
    ) -> anyhow::Result<Self> {
        /*
         * Initial setup
         */
        let connection = Connection::connect_to_env()?;

        let event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let c_focused_surface: Rc<RefCell<ClientFocus>> = Default::default();
        let c_hovered_surface: Rc<RefCell<ClientFocus>> = Default::default();
        let shm = ShmState::new();
        let multipool = MultiPool::new(&shm);
        let registry_state = RegistryState::new(&connection, &qh);

        let client_state = ClientState {
            cursor_surface: None,
            shm: None,

            focused_surface: c_focused_surface.clone(),
            hovered_surface: c_hovered_surface.clone(),

            _queue_handle: qh,
            connection,
            seat_state: SeatState::new(),
            output_state: OutputState::new(),
            compositor_state: CompositorState::new(),
            shm_state: shm,
            xdg_shell_state: XdgShellState::new(),
            registry_state,
            multipool: multipool.ok(),
        };

        // let _ = embedded_server_state
        //     .selected_data_provider
        //     .env_handle
        //     .set(env.clone());

        // TODO refactor to watch outputs and update space when outputs change or new outputs appear
        sctk::event_loop::WaylandSource::new(event_queue)
            .unwrap()
            .insert(loop_handle)
            .unwrap();

        space.setup(
            &client_state.compositor_state,
            &client_state.connection,
            dh.clone(),
            c_focused_surface.clone(),
            c_hovered_surface.clone(),
        );

        Ok(client_state)
    }
}
