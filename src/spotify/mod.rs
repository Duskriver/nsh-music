//! The Spotify web player, run and driven by the app: the browser it runs in
//! (`chrome`), the DevTools connection (`cdp`), the session that keeps them
//! (`session`) and what the bridge answers (`model`).

pub mod cdp;
pub mod chrome;
pub mod demo;
pub mod model;
pub mod session;

pub use model::*;
pub use session::{Event, Session, Status};
