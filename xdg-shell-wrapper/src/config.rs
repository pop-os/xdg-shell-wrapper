// SPDX-License-Identifier: MPL-2.0-only

use std::collections::HashMap;
use std::fs::File;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use smithay::reexports::wayland_protocols::wlr::unstable::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use xdg::BaseDirectories;

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub enum Anchor {
    TopLeft,
    BottomLeft,
    Left,
    TopRight,
    BottomRight,
    Right,
    Top,
    Bottom,
    Center,
}

impl From<zwlr_layer_surface_v1::Anchor> for Anchor {
    fn from(align: zwlr_layer_surface_v1::Anchor) -> Self {
        if align.contains(zwlr_layer_surface_v1::Anchor::Top)
            && align.contains(zwlr_layer_surface_v1::Anchor::Left)
        {
            Self::TopLeft
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Bottom)
            && align.contains(zwlr_layer_surface_v1::Anchor::Left)
        {
            Self::BottomLeft
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Left) {
            Self::Left
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Top)
            && align.contains(zwlr_layer_surface_v1::Anchor::Right)
        {
            Self::TopRight
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Bottom)
            && align.contains(zwlr_layer_surface_v1::Anchor::Right)
        {
            Self::BottomRight
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Right) {
            Self::Right
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Top) {
            Self::Top
        } else if align.contains(zwlr_layer_surface_v1::Anchor::Bottom) {
            Self::Bottom
        } else {
            Self::Center
        }
    }
}

impl Into<zwlr_layer_surface_v1::Anchor> for Anchor {
    fn into(self) -> zwlr_layer_surface_v1::Anchor {
        let mut anchor = zwlr_layer_surface_v1::Anchor::empty();
        match self {
            Self::TopLeft => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Top);
                anchor.insert(zwlr_layer_surface_v1::Anchor::Left);
            }
            Self::BottomLeft => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Bottom);
                anchor.insert(zwlr_layer_surface_v1::Anchor::Left);
            }
            Self::Left => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Left);
            }
            Self::TopRight => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Top);
                anchor.insert(zwlr_layer_surface_v1::Anchor::Right);
            }
            Self::BottomRight => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Bottom);
                anchor.insert(zwlr_layer_surface_v1::Anchor::Right);
            }
            Self::Right => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Right);
            }
            Self::Top => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Top);
            }
            Self::Bottom => {
                anchor.insert(zwlr_layer_surface_v1::Anchor::Bottom);
            }
            _ => {}
        };
        anchor
    }
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub enum Layer {
    Background,
    Bottom,
    Top,
    Overlay,
}

impl From<zwlr_layer_shell_v1::Layer> for Layer {
    fn from(layer: zwlr_layer_shell_v1::Layer) -> Self {
        match layer {
            zwlr_layer_shell_v1::Layer::Background => Self::Background,
            zwlr_layer_shell_v1::Layer::Bottom => Self::Bottom,
            zwlr_layer_shell_v1::Layer::Top => Self::Top,
            zwlr_layer_shell_v1::Layer::Overlay => Self::Overlay,
            _ => Self::Top,
        }
    }
}

impl Into<zwlr_layer_shell_v1::Layer> for Layer {
    fn into(self) -> zwlr_layer_shell_v1::Layer {
        match self {
            Self::Background => zwlr_layer_shell_v1::Layer::Background,
            Self::Bottom => zwlr_layer_shell_v1::Layer::Bottom,
            Self::Top => zwlr_layer_shell_v1::Layer::Top,
            Self::Overlay => zwlr_layer_shell_v1::Layer::Overlay,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub enum KeyboardInteractivity {
    None,
    Exclusive,
    OnDemand,
}

impl From<zwlr_layer_surface_v1::KeyboardInteractivity> for KeyboardInteractivity {
    fn from(kb: zwlr_layer_surface_v1::KeyboardInteractivity) -> Self {
        match kb {
            zwlr_layer_surface_v1::KeyboardInteractivity::None => Self::None,
            zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive => Self::Exclusive,
            zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand => Self::OnDemand,
            _ => Self::None,
        }
    }
}

impl Into<zwlr_layer_surface_v1::KeyboardInteractivity> for KeyboardInteractivity {
    fn into(self) -> zwlr_layer_surface_v1::KeyboardInteractivity {
        match self {
            Self::None => zwlr_layer_surface_v1::KeyboardInteractivity::None,
            Self::Exclusive => zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
            Self::OnDemand => zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct XdgWrapperConfig {
    pub anchor: Anchor,
    pub layer: Layer,
    pub keyboard_interactivity: KeyboardInteractivity,
    pub min_dimensions: Option<(u32, u32)>,
    pub max_dimensions: Option<(u32, u32)>,
    pub output: Option<String>,
    pub exec: String,
}

impl Default for XdgWrapperConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::Center,
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            min_dimensions: None,
            max_dimensions: None,
            output: None,
            exec: "".into(),
        }
    }
}

impl XdgWrapperConfig {
    pub fn load(name: &str) -> Self {
        match Self::get_configs().remove(name.into()) {
            Some(c) => c,
            _ => Self::default(),
        }
    }

    pub fn write(&self, name: &str) -> Result<()> {
        let mut configs = Self::get_configs();
        configs.insert(name.into(), XdgWrapperConfig::default());
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
                file.map(|file| ron::de::from_reader::<_, HashMap<String, XdgWrapperConfig>>(file?))
            }) {
            Ok(Some(Ok(c))) => c,
            _ => HashMap::new(),
        }
    }
}
