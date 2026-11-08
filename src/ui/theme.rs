//! The app's own tokens, beyond shadcn's (`theme.json`): each read through
//! one function, with a fallback that is visibly off if the theme lacks it.

use nsh_base::theme;
use nsh_core::prelude::*;

fn token(name: &str, fallback: fn() -> AdaptiveColor) -> AdaptiveColor {
    theme().color(name).unwrap_or_else(fallback)
}

/// Behind the panels: the window's own background.
pub fn shell() -> AdaptiveColor {
    token("shell", || theme().destructive)
}

/// The player bar.
pub fn player() -> AdaptiveColor {
    token("player", || theme().destructive)
}

/// The round play button and the progress thumb.
pub fn control() -> AdaptiveColor {
    token("control", || theme().destructive)
}

/// What is drawn on [`control`].
pub fn control_foreground() -> AdaptiveColor {
    token("control-foreground", || theme().destructive)
}

/// A hovered row of a list.
pub fn row_hover() -> AdaptiveColor {
    token("row-hover", || theme().destructive)
}

/// The corners of a card.
pub fn radius_card() -> Points {
    px(theme().number("radius-card").unwrap_or(3.0))
}

/// The corners of a panel (the sidebar, the main view).
pub fn radius_panel() -> Points {
    px(theme().number("radius-panel").unwrap_or(3.0))
}
