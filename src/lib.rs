pub mod config;
pub mod main_controller;

pub mod file_manager;
pub mod global_config;
pub mod utils;

pub use global_config::{get_global_config, init_global_config};
