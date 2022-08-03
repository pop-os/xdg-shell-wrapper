// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    output::{Mode as c_Mode, OutputHandler, OutputInfo, OutputState},
    reexports::{
        client::protocol::wl_output::Subpixel as c_Subpixel,
        client::{protocol::wl_output, Connection, QueueHandle},
    },
};
use slog::Logger;
use smithay::{
    reexports::wayland_server::{
        backend::GlobalId,
        protocol::wl_output::{Subpixel as s_Subpixel, Transform},
        DisplayHandle,
    },
    wayland::output::{Mode as s_Mode, Output, PhysicalProperties, Scale},
};
use xdg_shell_wrapper_config::WrapperConfig;

use crate::{
    client_state::ClientState, server_state::ServerState, shared_state::GlobalState,
    space::WrapperSpace,
};

impl<W: WrapperSpace> OutputHandler for GlobalState<W> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.client_state.output_state
    }

    fn new_output(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let info = match self.output_state().info(&output) {
            Some(info) if info.name.is_some() => info,
            _ => return,
        };

        let GlobalState {
            client_state: ClientState {
                compositor_state, ..
            },
            server_state: ServerState { display_handle, .. },
            space,
            log,
            ..
        } = self;

        let config = space.config();
        let configured_outputs = match config.outputs() {
            xdg_shell_wrapper_config::WrapperOutput::All => info.name.iter().cloned().collect(),
            xdg_shell_wrapper_config::WrapperOutput::Name(list) => list,
        };

        if configured_outputs
            .iter()
            .any(|configured| Some(configured) == info.name.as_ref())
        {
            // construct a surface for an output if possible
            let s_output = c_output_as_s_output::<W>(display_handle, &info, log.clone());

            space
                .handle_output(compositor_state, conn, Some(output), Some(s_output.0))
                .unwrap();
        }
    }

    fn update_output(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let info = match self.output_state().info(&output) {
            Some(info) if info.name.is_some() => info,
            _ => return,
        };
        let GlobalState {
            client_state: ClientState {
                compositor_state, ..
            },
            server_state: ServerState { display_handle, .. },
            space,
            log,
            ..
        } = self;

        let config = space.config();
        let configured_outputs = match config.outputs() {
            xdg_shell_wrapper_config::WrapperOutput::All => info.name.iter().cloned().collect(),
            xdg_shell_wrapper_config::WrapperOutput::Name(list) => list,
        };

        if configured_outputs
            .iter()
            .any(|configured| Some(configured) == info.name.as_ref())
        {
            // construct a surface for an output if possible
            let s_output = c_output_as_s_output::<W>(display_handle, &info, log.clone());

            space
                .handle_output(compositor_state, conn, Some(output), Some(s_output.0))
                .unwrap();
        }
    }

    fn output_destroyed(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let info = match self.output_state().info(&output) {
            Some(info) if info.name.is_some() => info,
            _ => return,
        };
        let GlobalState {
            client_state: ClientState {
                compositor_state, ..
            },
            server_state: ServerState { display_handle, .. },
            space,
            log,
            ..
        } = self;

        let config = space.config();
        let configured_outputs = match config.outputs() {
            xdg_shell_wrapper_config::WrapperOutput::All => info.name.iter().cloned().collect(),
            xdg_shell_wrapper_config::WrapperOutput::Name(list) => list,
        };

        if configured_outputs
            .iter()
            .any(|configured| Some(configured) == info.name.as_ref())
        {
            // construct a surface for an output if possible
            let s_output = c_output_as_s_output::<W>(display_handle, &info, log.clone());

            space
                .handle_output(compositor_state, conn, Some(output), Some(s_output.0))
                .unwrap();
        }
    }
}

/// convert client output to server output
pub fn c_output_as_s_output<W: WrapperSpace + 'static>(
    dh: &DisplayHandle,
    info: &OutputInfo,
    logger: Logger,
) -> (Output, GlobalId) {
    let s_output = Output::new(
        info.name.clone().unwrap_or_default(), // the name of this output,
        PhysicalProperties {
            size: info.physical_size.into(), // dimensions (width, height) in mm
            subpixel: match info.subpixel {
                c_Subpixel::None => s_Subpixel::None,
                c_Subpixel::HorizontalRgb => s_Subpixel::HorizontalRgb,
                c_Subpixel::HorizontalBgr => s_Subpixel::HorizontalBgr,
                c_Subpixel::VerticalRgb => s_Subpixel::VerticalRgb,
                c_Subpixel::VerticalBgr => s_Subpixel::VerticalBgr,
                _ => s_Subpixel::Unknown,
            }, // subpixel information
            make: info.make.clone(),         // make of the monitor
            model: info.model.clone(),       // model of the monitor
        },
        logger.clone(), // insert a logger here
    );
    for c_Mode {
        dimensions,
        refresh_rate,
        current,
        preferred,
    } in &info.modes
    {
        let s_mode = s_Mode {
            size: (*dimensions).into(),
            refresh: *refresh_rate,
        };
        if *preferred {
            s_output.set_preferred(s_mode);
        }
        if *current {
            s_output.change_current_state(
                Some(s_mode),
                Some(Transform::Normal),
                Some(Scale::Integer(1)),
                Some(info.location.into()),
            )
        }
        s_output.add_mode(s_mode);
    }
    let s_output_global = s_output.create_global::<GlobalState<W>>(dh);
    (s_output, s_output_global)
}
