//! Core file operations module.

mod archive;
mod bookmarks;
mod file_ops;
mod opener;

pub use archive::*;
pub use bookmarks::*;
pub use file_ops::*;
pub use opener::*;
pub mod job;
pub mod loader;

pub mod discovery;
pub mod editor;
pub mod git_review;
pub mod preview;
#[cfg(feature = "terminal")]
pub mod terminal;
