use crate::error::Error;
use crate::state::AppState;

pub mod autocomplete;
pub mod commands;
pub mod utils;

pub struct Data {
    pub app_state: AppState,
} // User data, which is stored and accessible in all command invocations
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Get all available commands
pub fn get_commands() -> Vec<poise::Command<Data, Error>> {
    vec![
        commands::search(),
        commands::terms(),
        commands::ics(),
        commands::gcal(),
    ]
}
