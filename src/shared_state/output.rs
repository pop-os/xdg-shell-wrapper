// SPDX-License-Identifier: GPL-3.0-only

use std::cell::RefCell;
use std::rc::Rc;

use sctk::environment::Environment;
use sctk::{
    output::{Mode as c_Mode, OutputInfo},
    reexports::{
        client::protocol::{
            wl_output::{self as c_wl_output, Subpixel as c_Subpixel},
            wl_pointer as c_wl_pointer, wl_surface as c_wl_surface,
        },
        client::{self, protocol::wl_keyboard, Attached, Display},
    },
};
use slog::Logger;
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use smithay::reexports::wayland_server::{
    protocol::{
        wl_output::{Subpixel as s_Subpixel, WlOutput as s_WlOutput},
        wl_surface::WlSurface,
    },
    Display as s_Display,
};
use smithay::wayland::output::{Mode as s_Mode, Output as s_Output, PhysicalProperties};

use crate::client::Env;
use crate::{OutputGroup, Surface, XdgWrapperConfig};

pub fn handle_output(
    config: XdgWrapperConfig,
    layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    env_handle: Environment<Env>,
    surface_handle: Rc<RefCell<Option<(u32, Surface)>>>,
    logger: Logger,
    display_: Display,
    output: client::protocol::wl_output::WlOutput,
    info: &OutputInfo,
    server_display: &mut s_Display,
    s_outputs: &mut Vec<OutputGroup>,
) {
    // remove output with id if obsolete
    // add output to list if new output
    // if no output in handle after removing output, replace with first output from list
    let mut handle = surface_handle.borrow_mut();
    if info.obsolete {
        // an output has been removed, release it
        if handle.as_ref().filter(|(i, _)| *i != info.id).is_some() {
            *handle = None;
        }

        // remove outputs from embedded server when they are removed from the client
        for (_, global_output, _, _) in s_outputs.drain_filter(|(_, _, i, _)| *i != info.id) {
            global_output.destroy();
        }

        output.release();
    } else {
        // Create the Output for the server with given name and physical properties
        let (s_output, _s_output_global) = s_Output::new(
            server_display,    // the display
            info.name.clone(), // the name of this output,
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
            is_preferred,
            ..
        } in &info.modes
        {
            let s_mode = s_Mode {
                size: dimensions.clone().into(),
                refresh: *refresh_rate,
            };
            if *is_preferred {
                s_output.set_preferred(s_mode);
            } else {
                s_output.add_mode(s_mode);
            }
        }
    }
    if handle.is_none() {
        if let Some((_, _, _, output)) = s_outputs.first() {
            // construct a surface for an output if possible
            let surface = env_handle.create_surface().detach();
            let pool = env_handle
                .create_auto_pool()
                .expect("Failed to create a memory pool!");
            *handle = Some((
                info.id,
                Surface::new(
                    output,
                    surface,
                    &layer_shell.clone(),
                    pool,
                    config.clone(),
                    logger.clone(),
                    display_.clone(),
                ),
            ));
        }
    }
}
