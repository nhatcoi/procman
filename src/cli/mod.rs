pub mod args;
pub mod commands;
pub mod format;

use anyhow::Result;

use args::{Cli, Commands};
use commands::updater::{check_for_updates_background, get_cached_update_banner};

pub fn dispatch(cli: Cli) -> Result<()> {
    check_for_updates_background();

    let res = match cli.command {
        Some(Commands::Start { name, forward }) => commands::start::execute(name, forward),
        Some(Commands::Stop { name }) => commands::stop::execute_stop(name),
        Some(Commands::Kill { name }) => commands::stop::execute_kill(name),
        Some(Commands::Restart { name, forward }) => commands::stop::execute_restart(name, forward),
        Some(Commands::Status { name }) => commands::status::execute_status(name),
        Some(Commands::Logs {
            name,
            follow,
            lines,
        }) => commands::logs::execute(name, follow, lines),
        Some(Commands::Watch { name }) => commands::watch::execute(name),
        Some(Commands::Forward { name }) => commands::status::execute_forward(name),
        Some(Commands::Unforward { name }) => commands::status::execute_unforward(name),
        Some(Commands::Ui { all }) => crate::tui::render_ui(all),
        Some(Commands::Ps) => commands::status::execute_ps(),
        Some(Commands::Upgrade { tag }) => commands::updater::execute(tag),
        Some(Commands::Uninstall { yes, purge }) => commands::uninstaller::execute(yes, purge),
        Some(Commands::Skill { dir, force }) => commands::skill::execute(&dir, force),
        Some(Commands::Doctor {
            name,
            spot,
            ai,
            agent,
            fix,
        }) => commands::doctor::execute(name, spot, ai, agent, fix),
        Some(Commands::Init {
            dir,
            ai,
            agent,
            yes,
            force,
        }) => commands::init::execute(dir, ai, agent, yes, force),
        Some(Commands::Mcp { dir }) => commands::mcp::execute(dir),
        None => commands::status::execute_status(None),

    };

    if let Some(banner) = get_cached_update_banner() {
        eprintln!("{}", banner);
    }

    res
}
