use crate::state::AppState;
use anyhow::Error;
use std::fmt::Write;

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

/// Build a deterministic fingerprint of command definitions for change detection.
///
/// Includes command names, descriptions, and parameter metadata. If any of these
/// change, the fingerprint changes, triggering re-registration with Discord.
pub fn commands_fingerprint(commands: &[poise::Command<Data, Error>]) -> String {
    let mut parts: Vec<String> = commands
        .iter()
        .map(|cmd| {
            let mut s = String::new();
            write!(s, "{}:", cmd.name).unwrap();
            if let Some(desc) = &cmd.description {
                write!(s, "{desc}").unwrap();
            }
            s.push('(');
            let params: Vec<String> = cmd
                .parameters
                .iter()
                .map(|p| {
                    let mut ps = p.name.clone();
                    if let Some(desc) = &p.description {
                        write!(ps, ":{desc}").unwrap();
                    }
                    write!(ps, ":{}", p.required).unwrap();
                    ps
                })
                .collect();
            s.push_str(&params.join(","));
            s.push(')');
            s
        })
        .collect();
    parts.sort();
    parts.join(";")
}
