pub mod backend;
pub mod cli;

#[cfg(feature = "gui")]
pub mod gui;

use std::process::ExitCode;

pub fn run() -> ExitCode {
    cli::run()
}
