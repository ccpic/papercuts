pub mod add;
pub mod block;
pub mod doctor;
mod ids;
pub mod list;
pub mod rebaseline;
pub mod resolve;
pub mod schema;
mod status_transition;
pub mod unblock;

use crate::cli::{Cli, Command};
use crate::error::{AppError, AppResult};
use crate::output;
use jiff::Timestamp;

pub fn run(cli: Cli, now: Timestamp) -> AppResult<i32> {
    match cli.command {
        Command::Add(args) => add::run(args, cli.file, cli.pretty, now),
        Command::List(args) => list::run(args, cli.file, cli.pretty, now),
        Command::Resolve(args) => resolve::run(args, cli.file, cli.pretty, now),
        Command::Rebaseline(args) => rebaseline::run(args, cli.file, cli.pretty, now),
        Command::Block(args) => block::run(args, cli.file, cli.pretty, now),
        Command::Unblock(args) => unblock::run(args, cli.file, cli.pretty, now),
        Command::Schema { target } => {
            output::write_success(schema::contract(target), cli.pretty, output::Meta::new())
                .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
            Ok(0)
        }
        Command::Doctor => doctor::run(cli.file, cli.pretty),
    }
}
