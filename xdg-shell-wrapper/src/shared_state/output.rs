// SPDX-License-Identifier: MPL-2.0-only

use crate::{client::Env, OutputGroup, WrapperRenderer, XdgWrapperConfig};
use sctk::{
    environment::Environment,
    output::{Mode as c_Mode, OutputInfo},
    reexports::{
        client::protocol::wl_output::Subpixel as c_Subpixel,
        client::{self, Attached, Display},
    },
};
use slog::Logger;
use smithay::{
    reexports::{
        wayland_protocols::wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1,
        wayland_server::{protocol::wl_output::Subpixel as s_Subpixel, Display as s_Display},
    },
    wayland::output::{Mode as s_Mode, Output as s_Output, PhysicalProperties},
};

pub fn handle_output(
    config: XdgWrapperConfig,
    layer_shell: &Attached<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    env_handle: Environment<Env>,
    renderer_handle: &mut Option<WrapperRenderer>,
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
    if info.obsolete {
        // an output has been removed, release it
        if renderer_handle
            .as_ref()
            .filter(|r| r.output_id != info.id)
            .is_some()
        {
            *renderer_handle = None;
        }

        // TODO replace with drain_filter
        let mut i = 0;
        while i < s_outputs.len() {
            let id = s_outputs[i].2;
            if ! info.id == id {
                let removed = s_outputs.remove(i);
                removed.1.destroy();
            } else {
                i += 1;
            }
        }

        output.release();
    } else {
        // Create the Output for the server with given name and physical properties
        let (s_output, s_output_global) = s_Output::new(
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
        s_outputs.push((s_output, s_output_global, info.id, output));
    }
    if renderer_handle.is_none() {
        if let Some((_, _, _, output)) = s_outputs.first() {
            // construct a surface for an output if possible
            let pool = env_handle
                .create_auto_pool()
                .expect("Failed to create a memory pool!");
            *renderer_handle = Some(WrapperRenderer::new(
                output.clone(),
                info.id,
                pool,
                config.clone(),
                display_.clone(),
                layer_shell.clone(),
                logger.clone(),
            ));
        }
    }
}
