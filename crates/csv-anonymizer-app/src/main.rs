mod app_logic;
mod cli;

use cli::{CliAction, parse_cli_args, print_help, run_cli};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    match parse_cli_args(std::env::args_os().skip(1))? {
        CliAction::Help => {
            print_help();
            Ok(())
        }
        CliAction::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        action => run_cli(action),
    }
}
