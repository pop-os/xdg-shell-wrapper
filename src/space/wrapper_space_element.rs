use smithay::{
    desktop::{space::space_elements, PopupKind, Window},
    wayland::shell::xdg::PopupSurface,
};

space_elements! {
    pub WrapperSpaceElement;
    Window=Window,
}
