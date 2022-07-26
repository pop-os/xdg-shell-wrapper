// SPDX-License-Identifier: MPL-2.0-only

use sctk::{
    environment::Environment,
    output::{Mode as c_Mode, OutputInfo},
    reexports::{
        client::protocol::wl_output::Subpixel as c_Subpixel,
        client::{self},
    },
};
use slog::Logger;
use smithay::{
    reexports::wayland_server::{
        self,
        backend::GlobalId,
        protocol::wl_output::{Subpixel as s_Subpixel, Transform},
        DisplayHandle,
    },
    wayland::output::{Mode as s_Mode, Output as s_Output, PhysicalProperties, Scale},
};

use crate::{
    shared_state::{GlobalState, OutputGroup},
    space::WrapperSpace,
};

use super::super::state::Env;

pub fn handle_output<W: WrapperSpace + 'static>(
    env_handle: &Environment<Env>,
    logger: Logger,
    output: &client::protocol::wl_output::WlOutput,
    info: &OutputInfo,
    dh: &mut DisplayHandle,
    s_outputs: &mut Vec<OutputGroup>,
    space: &mut W,
) {
    // remove output with id if obsolete
    // add output to list if new output

    if info.obsolete {
        // an output has been removed, release it
        // this should not be reached
        output.release();
    } else {
        // Create the Output for the server with given name and physical properties
        let (s_output, s_output_global) = c_output_as_s_output::<W>(dh, info, logger.clone());
        s_outputs.push((s_output, s_output_global, info.name.clone(), output.clone()));
    }

    // construct a surface for an output if possible
    space
        .handle_output(dh.clone(), env_handle, Some(output), Some(info))
        .unwrap();
}

pub fn c_output_as_s_output<W: WrapperSpace + 'static>(
    dh: &DisplayHandle,
    info: &OutputInfo,
    logger: Logger,
) -> (s_Output, GlobalId) {
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
        is_current,
    } in &info.modes
    {
        let s_mode = s_Mode {
            size: (*dimensions).into(),
            refresh: *refresh_rate,
        };
        if *is_preferred {
            s_output.set_preferred(s_mode);
        }
        if *is_current {
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
