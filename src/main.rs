#![warn(clippy::correctness)]
#![warn(clippy::arithmetic_side_effects)]
#![warn(clippy::assertions_on_result_states)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::deref_by_slicing)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::error_impl_error)]
#![warn(clippy::filetype_is_file)]
#![warn(clippy::float_cmp_const)]
#![warn(clippy::float_cmp)]
#![warn(clippy::format_push_string)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::integer_division)]
#![warn(clippy::lossy_float_literal)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::mixed_read_write_in_expression)]
#![warn(clippy::multiple_inherent_impl)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_type_annotations)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::try_err)]
#![warn(clippy::must_use_candidate)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::manual_let_else)]
#![warn(clippy::manual_ok_or)]
#![warn(clippy::manual_string_new)]
#![warn(clippy::nursery)]

use anyhow::Result;
use master_of_puppets::{master_of_puppets::MasterOfPuppets, puppet::PuppetBuilder};
use tokio::signal;
use tracing::info;

pub mod cli;
pub mod cmd;
pub mod portfolio;
pub mod puppet;
pub mod server;

use crate::{
    cli::CliExt,
    puppet::{db::Db, degiro::Degiro},
};

#[derive(Debug, Clone)]
pub struct App {}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {}
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().pretty().init();
    info!("Starting Vogelsang...");

    let app = App::new();
    app.run().await.unwrap();
    Ok(())
}
