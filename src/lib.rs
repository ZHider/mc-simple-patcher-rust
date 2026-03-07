pub mod config;
pub mod generator;
pub mod main_controller;

pub mod file_manager;
pub mod global_config;
pub mod utils;

pub use global_config::{get_global_config, set_global_config};
