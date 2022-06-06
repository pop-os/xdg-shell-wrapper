// SPDX-License-Identifier: MPL-2.0-only

use std::collections::HashMap;
use std::fs::File;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use xdg::BaseDirectories;
use cosmic_panel_config::config::*;
use std::ops::Range;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub anchor: Anchor,
    pub layer: Layer,
    pub keyboard_interactivity: KeyboardInteractivity,
    pub width_range: Option<Range<u32>>,
    pub height_range: Option<Range<u32>>,
    pub output: CosmicPanelOutput,
    pub exec: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            anchor: Anchor::Center,
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            width_range: None,
            height_range: None,
            output: CosmicPanelOutput::Auto,
            exec: "".into(),
        }
    }
}

impl Config {
    pub fn load(name: &str) -> Self {
        match Self::get_configs().remove(name.into()) {
            Some(c) => c,
            _ => Self::default(),
        }
    }

    pub fn write(&self, name: &str) -> Result<()> {
        let mut configs = Self::get_configs();
        configs.insert(name.into(), Config::default());
        let xdg = BaseDirectories::new()?;
        let f = xdg
            .place_config_file("xdg-shell-wrapper/config.ron")
            .unwrap();
        let f = File::create(f)?;
        ron::ser::to_writer_pretty(&f, &configs, ron::ser::PrettyConfig::default())?;
        return Ok(());
    }

    fn get_configs() -> HashMap<String, Self> {
        match BaseDirectories::new()
            .map(|dirs| dirs.find_config_file("xdg-shell-wrapper/config.ron"))
            .map(|c| c.map(|c| File::open(c)))
            .map(|file| {
                file.map(|file| ron::de::from_reader::<_, HashMap<String, Config>>(file?))
            }) {
            Ok(Some(Ok(c))) => c,
            _ => HashMap::new(),
        }
    }
}


impl XdgWrapperConfig for Config {
    fn plugins_center(&self) -> Option<Vec<(String, u32)>> {
        Some(vec![(self.exec.clone(), 1000)])
    }
    fn output(&self) -> CosmicPanelOutput {
        self.output.clone()
    }

    fn anchor(&self) -> Anchor {
        self.anchor
    }

    fn padding(&self) -> u32 {
        0
    }

    fn layer(&self) -> zwlr_layer_shell_v1::Layer {
        self.layer.into()
    }

    fn keyboard_interactivity(&self) -> zwlr_layer_surface_v1::KeyboardInteractivity {
        self.keyboard_interactivity.into()
    }

    /// get constraints for the thickness of the panel bar
    fn get_dimensions(&self, output_dims: (u32, u32)) -> (Option<Range<u32>>, Option<Range<u32>>) {
        (self.width_range.clone(), self.height_range.clone())
    }
}

