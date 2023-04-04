use std::time::Duration;
use std::{cell::RefCell, rc::Rc, time::Instant};

use sctk::reexports::client::WaylandSource;
use sctk::shell::wlr_layer::LayerSurface;
use sctk::shell::{wlr_layer::LayerShell, xdg::XdgShell};
use sctk::shm::Shm;
use sctk::{
    compositor::CompositorState,
    output::OutputState,
    reexports::client::{
        globals::registry_queue_init,
        protocol::{
            wl_keyboard,
            wl_output::WlOutput,
            wl_pointer,
            wl_seat::WlSeat,
            wl_surface::{self, WlSurface},
        },
        Connection, QueueHandle,
    },
    registry::RegistryState,
    seat::SeatState,
    shm::multi::MultiPool,
};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles2::Gles2Renderer;
use smithay::backend::renderer::{Bind, Unbind};
use smithay::{
    backend::egl::EGLSurface,
    desktop::LayerSurface as SmithayLayerSurface,
    output::Output,
    reexports::{calloop, wayland_server::backend::GlobalId},
};

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
    pub shm_state: Shm,
    /// state
    pub xdg_shell_state: XdgShell,
    /// state
    pub layer_state: LayerShell,

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
    pub(crate) outputs: Vec<(WlOutput, Output, GlobalId)>,

    pub(crate) proxied_layer_surfaces: Vec<(
        Rc<EGLSurface>,
        OutputDamageTracker,
        SmithayLayerSurface,
        LayerSurface,
        SurfaceState,
    )>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SurfaceState {
    WaitingFirst,
    Waiting,
    Dirty,
}

impl<W: WrapperSpace + 'static> ClientState<W> {
    pub(crate) fn new(
        loop_handle: calloop::LoopHandle<'static, GlobalState<W>>,
        space: &mut W,
        _embedded_server_state: &mut ServerState<W>,
    ) -> anyhow::Result<Self> {
        /*
         * Initial setup
         */
        let connection = Connection::connect_to_env()?;

        let (globals, event_queue) = registry_queue_init(&connection).unwrap();
        let qh = event_queue.handle();
        let registry_state = RegistryState::new(&globals);

        let client_state = ClientState {
            focused_surface: space.get_client_focused_surface(),
            hovered_surface: space.get_client_hovered_surface(),
            proxied_layer_surfaces: Vec::new(),

            queue_handle: qh.clone(),
            connection,
            seat_state: SeatState::new(&globals, &qh),
            output_state: OutputState::new(&globals, &qh),
            compositor_state: CompositorState::bind(&globals, &qh)
                .expect("wl_compositor not available"),
            shm_state: Shm::bind(&globals, &qh).expect("wl_shm not available"),
            xdg_shell_state: XdgShell::bind(&globals, &qh).expect("xdg shell not available"),
            layer_state: LayerShell::bind(&globals, &qh).expect("layer shell is not available"),

            outputs: Default::default(),
            registry_state,
            multipool: None,
            cursor_surface: None,
            last_key_pressed: Vec::new(),
        };

        // TODO refactor to watch outputs and update space when outputs change or new outputs appear
        WaylandSource::new(event_queue)
            .unwrap()
            .insert(loop_handle)
            .unwrap();

        Ok(client_state)
    }

    /// draw the proxied layer shell surfaces
    pub fn draw_layer_surfaces(&mut self, renderer: &mut Gles2Renderer, time: u32) {
        let clear_color = &[0.0, 0.0, 0.0, 0.0];
        for (egl_surface, dmg_tracked_renderer, s_layer, c_layer, state) in
            &mut self.proxied_layer_surfaces
        {
            match state {
                SurfaceState::WaitingFirst => continue,
                SurfaceState::Waiting => continue,
                SurfaceState::Dirty => {}
            };
            let _ = renderer.unbind();
            let _ = renderer.bind(egl_surface.clone());
            let elements: Vec<WaylandSurfaceRenderElement<Gles2Renderer>> =
                s_layer.render_elements(renderer, (0, 0).into(), 1.0.into());
            dmg_tracked_renderer
                .render_output(
                    renderer,
                    egl_surface.buffer_age().unwrap_or_default() as usize,
                    &elements,
                    *clear_color,
                )
                .unwrap();
            egl_surface.swap_buffers(None).unwrap();
            // FIXME: damage tracking issues on integrated graphics but not nvidia
            // self.egl_surface
            //     .as_ref()
            //     .unwrap()
            //     .swap_buffers(res.0.as_deref_mut())?;

            renderer.unbind().unwrap();
            // TODO what if there is "no output"?
            for o in &self.outputs {
                let output = &o.1;
                s_layer.send_frame(
                    &o.1,
                    Duration::from_millis(time as u64),
                    None,
                    move |_, _| Some(output.clone()),
                )
            }
            *state = SurfaceState::Waiting;
        }
    }
}
