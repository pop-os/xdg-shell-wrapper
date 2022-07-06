use std::{cell::RefCell, rc::Rc};

use once_cell::sync::OnceCell;
use sctk::reexports::client::Attached;
use slog::{Logger, info};
use smithay::{
    desktop::Window,
    reexports::wayland_server::{protocol::wl_surface::WlSurface, DisplayHandle},
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        output::OutputManagerState,
        seat::{self, SeatState},
        shell::xdg::XdgShellState,
        shm::ShmState, dmabuf::{DmabufState, DmabufGlobal},
    }, backend::renderer::{gles2::Gles2Renderer, ImportEgl, ImportDma},
};

use crate::{
    client_state::{ClientSeat, Env},
    shared_state::{GlobalState, SelectedDataProvider},
    space::WrapperSpace,
};

#[derive(Debug)]
pub(crate) struct EmbeddedServerState<W: WrapperSpace + 'static> {
    pub(crate) root_window: Option<Rc<RefCell<Window>>>,
    pub(crate) focused_surface: Rc<RefCell<Option<WlSurface>>>,
    pub(crate) selected_data_provider: SelectedDataProvider,
    pub(crate) last_button: Option<u32>,
    pub(crate) seats: Vec<SeatPair<W>>,

    // Smithay State
    pub(crate) compositor_state: CompositorState,
    pub(crate) xdg_shell_state: XdgShellState,
    pub(crate) shm_state: ShmState,
    pub(crate) output_manager_state: OutputManagerState,
    pub(crate) seat_state: SeatState<GlobalState<W>>,
    pub(crate) data_device_state: DataDeviceState,
    pub(crate) dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
}

impl<W: WrapperSpace> EmbeddedServerState<W> {
    pub(crate) fn new(dh: &DisplayHandle, log: Logger) -> EmbeddedServerState<W> {
        let selected_seat: Rc<
            RefCell<Option<Attached<sctk::reexports::client::protocol::wl_seat::WlSeat>>>,
        > = Rc::new(RefCell::new(None));
        let env: Rc<OnceCell<sctk::environment::Environment<Env>>> = Rc::new(OnceCell::new());
        let selected_data_provider = selected_seat.clone();
        let env_handle = env.clone();
        let logger = log.clone();
        // init_data_device(
        //     &mut display,
        //     move |event| {
        //         /* a callback to react to client DnD/selection actions */
        //         match event {
        //             DataDeviceEvent::SendSelection { mime_type, fd } => {
        //                 if let (Some(seat), Some(env_handle)) =
        //                     (selected_data_provider.borrow().as_ref(), env_handle.get())
        //                 {
        //                     let res = env_handle.with_data_device(seat, |data_device| {
        //                         data_device.with_selection(|offer| {
        //                             if let Some(offer) = offer {
        //                                 offer.with_mime_types(|types| {
        //                                     if types.contains(&mime_type) {
        //                                         let _ = unsafe { offer.receive_to_fd(mime_type, fd) };
        //                                     }
        //                                 })
        //                             }
        //                         })
        //                     });

        //                     if let Err(err) = res {
        //                         error!(logger, "{:?}", err);
        //                     }
        //                 }
        //             }
        //             DataDeviceEvent::DnDStarted {
        //                 source: _,
        //                 icon: _,
        //                 seat: _,
        //             } => {
        //                 // dbg!(source);
        //                 // dbg!(icon);
        //                 // dbg!(seat);
        //             }

        //             DataDeviceEvent::DnDDropped { seat: _ } => {
        //                 // dbg!(seat);
        //             }
        //             DataDeviceEvent::NewSelection(_) => {}
        //         };
        //     },
        //     default_action_chooser,
        //     log.clone(),
        // );

        EmbeddedServerState {
            root_window: Default::default(),
            focused_surface: Default::default(),
            selected_data_provider: SelectedDataProvider {
                seat: selected_seat,
                env_handle: env,
            },
            last_button: None,
            seats: Vec::new(),
            compositor_state: CompositorState::new::<GlobalState<W>, _>(&dh, log.clone()),
            xdg_shell_state: XdgShellState::new::<GlobalState<W>, _>(&dh, log.clone()),
            shm_state: ShmState::new::<GlobalState<W>, _>(&dh, vec![], log.clone()),
            output_manager_state: OutputManagerState::new_with_xdg_output::<GlobalState<W>>(&dh),
            seat_state: SeatState::new(),
            data_device_state: DataDeviceState::new::<GlobalState<W>, _>(&dh, log.clone()),
            dmabuf_state: None,
        }
    }
}



#[derive(Debug)]
pub(crate) struct SeatPair<W: WrapperSpace + 'static> {
    pub(crate) name: String,
    pub(crate) client: ClientSeat,
    pub(crate) server: seat::Seat<GlobalState<W>>,
}
