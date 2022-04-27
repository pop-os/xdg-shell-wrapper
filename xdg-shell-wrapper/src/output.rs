// SPDX-License-Identifier: MPL-2.0-only

use crate::{
    client::Env,
    render::{self, WrapperRenderer},
    OutputGroup, XdgWrapperConfig,
};
use sctk::{
    environment::Environment,
    output::{with_output_info, Mode as c_Mode, OutputInfo},
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
    let preferred_output = config.output.as_ref().unwrap();

    let mut needs_new_output = &info.name == preferred_output;
    if info.obsolete {
        // an output has been removed, release it
        needs_new_output = renderer_handle
            .as_ref()
            .filter(|r| r.output.as_ref().unwrap().1 != info.name)
            .is_some();

        // TODO replace with drain_filter
        let mut i = 0;
        while i < s_outputs.len() {
            let name = &s_outputs[i].2;
            if &info.name != name {
                let removed = s_outputs.remove(i);
                removed.1.destroy();
            } else {
                i += 1;
            }
        }

        output.release();
    } else {
        // Create the Output for the server with given name and physical properties
        let s_output = s_Output::new(
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
        let s_output_global = s_output.create_global(server_display);
        s_outputs.push((s_output, s_output_global, info.name.clone(), output));
    }
    let new_output = if let Some(preferred_output_index) = s_outputs
        .iter()
        .position(|(_, _, name, _)| name == preferred_output)
    {
        Some((
            s_outputs[preferred_output_index].3.clone(),
            preferred_output.clone(),
        ))
    } else {
        None
    };
    if renderer_handle.is_none() {
        // construct a surface for an output if possible
        let pool = env_handle
            .create_auto_pool()
            .expect("Failed to create a memory pool!");
        *renderer_handle = Some(WrapperRenderer::new(
            new_output,
            pool,
            config.clone(),
            display_.clone(),
            layer_shell.clone(),
            logger.clone(),
        ));
    } else if needs_new_output {
        renderer_handle.as_mut().unwrap().set_output(new_output);
    }
}
