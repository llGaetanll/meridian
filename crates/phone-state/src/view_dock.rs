//! The dock, and which tab is lit.

use crate::Screen;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDock {
    pub items: Vec<ViewDockItem>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ViewDockItem {
    pub screen: Screen,
    pub label: String,
    /// An icon name the view resolves in its own asset catalogue.
    pub icon: String,
    pub selected: bool,
}
