//! The app's icons: one family of 24 × 24 line glyphs (Lucide's geometry),
//! drawn as inline SVG with `currentColor`, so they take the text color of
//! the element they are in.

use nsh_core::{element::ImageSrc, prelude::*};

/// A glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Home,
    Search,
    Library,
    Play,
    Pause,
    SkipBack,
    SkipForward,
    Shuffle,
    Repeat,
    RepeatOne,
    Heart,
    HeartFilled,
    Volume,
    VolumeLow,
    VolumeMute,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Music,
    User,
    LogIn,
    LogOut,
    Disc,
    Clock,
    Alert,
    Close,
}

const STROKE: &str = r#"fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round""#;

fn body(icon: Icon) -> &'static str {
    match icon {
        Icon::Home => r#"<path d="M3 10.5 12 3l9 7.5"/><path d="M5 9.5V20a1 1 0 0 0 1 1h4v-6h4v6h4a1 1 0 0 0 1-1V9.5"/>"#,
        Icon::Search => r#"<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>"#,
        Icon::Library => r#"<path d="M4 4v16"/><path d="M9 4v16"/><path d="m14 4.5 5.5 15"/>"#,
        Icon::Play => r#"<path d="M7 4.5v15a.8.8 0 0 0 1.2.7l12-7.5a.8.8 0 0 0 0-1.4l-12-7.5A.8.8 0 0 0 7 4.5Z" fill="currentColor"/>"#,
        Icon::Pause => r#"<rect x="6" y="4" width="4" height="16" rx="1" fill="currentColor"/><rect x="14" y="4" width="4" height="16" rx="1" fill="currentColor"/>"#,
        Icon::SkipBack => r#"<path d="M19 20 9 12l10-8v16Z" fill="currentColor"/><path d="M5 19V5"/>"#,
        Icon::SkipForward => r#"<path d="m5 4 10 8-10 8V4Z" fill="currentColor"/><path d="M19 5v14"/>"#,
        Icon::Shuffle => r#"<path d="M2 18h1.4a4 4 0 0 0 3.3-1.8l6.6-9.4A4 4 0 0 1 16.6 5H22"/><path d="m18 2 4 3-4 3"/><path d="M2 6h1.4a4 4 0 0 1 3.3 1.8l.8 1.2"/><path d="M22 19h-5.4a4 4 0 0 1-3.3-1.8l-.8-1.2"/><path d="m18 16 4 3-4 3"/>"#,
        Icon::Repeat => r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>"#,
        Icon::RepeatOne => r#"<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/><path d="M11 10h1v4"/>"#,
        Icon::Heart => r#"<path d="M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z"/>"#,
        Icon::HeartFilled => r#"<path d="M19 14c1.5-1.5 3-3.2 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.8 0-3 .5-4.5 2-1.5-1.5-2.7-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4 3 5.5l7 7Z" fill="currentColor"/>"#,
        Icon::Volume => r#"<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.4 18.4a9 9 0 0 0 0-12.8"/>"#,
        Icon::VolumeLow => r#"<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="M16 9a5 5 0 0 1 0 6"/>"#,
        Icon::VolumeMute => r#"<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A2 2 0 0 1 5 8.2H3a1 1 0 0 0-1 1v5.6a1 1 0 0 0 1 1h2a2 2 0 0 1 1.4.6l3.4 3.4a.7.7 0 0 0 1.2-.5Z"/><path d="m22 9-6 6"/><path d="m16 9 6 6"/>"#,
        Icon::ChevronLeft => r#"<path d="m15 18-6-6 6-6"/>"#,
        Icon::ChevronRight => r#"<path d="m9 18 6-6-6-6"/>"#,
        Icon::ChevronDown => r#"<path d="m6 9 6 6 6-6"/>"#,
        Icon::Music => r#"<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>"#,
        Icon::User => r#"<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>"#,
        Icon::LogIn => r#"<path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><path d="m10 17 5-5-5-5"/><path d="M15 12H3"/>"#,
        Icon::LogOut => r#"<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5"/><path d="M21 12H9"/>"#,
        Icon::Disc => r#"<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>"#,
        Icon::Clock => r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#,
        Icon::Alert => r#"<circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/>"#,
        Icon::Close => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
    }
}

impl Icon {
    /// The glyph's markup.
    pub fn markup(self) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" {STROKE}>{}</svg>"#, body(self))
    }
}

/// The glyph at `size` points, in the text color around it.
pub fn icon(glyph: Icon, size: f32) -> Svg {
    svg(ImageSrc::svg_markup(&glyph.markup())).size(px(size))
}

/// A glyph that follows `glyph` (a play button's play or pause).
pub fn icon_with(glyph: impl Fn() -> Icon + 'static, size: f32) -> Svg {
    svg(move || ImageSrc::svg_markup(&glyph().markup())).size(px(size))
}
