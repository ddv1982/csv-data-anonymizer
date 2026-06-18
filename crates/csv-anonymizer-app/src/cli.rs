use crate::app_logic::should_auto_select;
use csv_anonymizer_core::{AnonymizeParams, AnonymizerService, PreviewParams};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliAction {
    Gui,
    Help,
    Version,
    Analyze {
        input: PathBuf,
    },
    SmokeAnonymize {
        input: PathBuf,
        output: PathBuf,
    },
    Anonymize {
        input: PathBuf,
        output: PathBuf,
        columns: Vec<usize>,
        deterministic: bool,
        seed: String,
        force: bool,
    },
}

pub(crate) fn parse_cli_args(
    args: impl IntoIterator<Item = OsString>,
) -> Result<CliAction, String> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(CliAction::Gui);
    }

    let command = args[0].to_string_lossy();
    if args.len() == 1 && command.starts_with("-psn_") {
        return Ok(CliAction::Gui);
    }

    match command.as_ref() {
        "--help" | "-h" | "help" => Ok(CliAction::Help),
        "--version" | "-V" | "version" => Ok(CliAction::Version),
        "--smoke-anonymize" => {
            if args.len() != 3 {
                return Err("--smoke-anonymize requires <input> <output>".to_string());
            }
            Ok(CliAction::SmokeAnonymize {
                input: PathBuf::from(&args[1]),
                output: PathBuf::from(&args[2]),
            })
        }
        "analyze" => {
            if args.len() != 2 {
                return Err("analyze requires <input>".to_string());
            }
            Ok(CliAction::Analyze {
                input: PathBuf::from(&args[1]),
            })
        }
        "anonymize" => parse_anonymize_args(&args[1..]),
        _ => Err(format!(
            "unknown command '{command}'. Use --help for supported commands."
        )),
    }
}

fn parse_anonymize_args(args: &[OsString]) -> Result<CliAction, String> {
    let mut input = None;
    let mut output = None;
    let mut columns = None;
    let mut deterministic = false;
    let mut seed = String::new();
    let mut force = false;
    let mut index = 0;

    while index < args.len() {
        let flag = args[index].to_string_lossy();
        match flag.as_ref() {
            "--input" | "-i" => {
                index += 1;
                input = args.get(index).map(PathBuf::from);
            }
            "--output" | "-o" => {
                index += 1;
                output = args.get(index).map(PathBuf::from);
            }
            "--columns" | "-c" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--columns requires a comma-separated value".to_string())?
                    .to_string_lossy();
                columns = Some(parse_columns(&value)?);
            }
            "--deterministic" => deterministic = true,
            "--seed" => {
                index += 1;
                seed = args
                    .get(index)
                    .ok_or_else(|| "--seed requires a value".to_string())?
                    .to_string_lossy()
                    .to_string();
            }
            "--force" => force = true,
            _ => return Err(format!("unknown anonymize option '{flag}'")),
        }
        index += 1;
    }

    Ok(CliAction::Anonymize {
        input: input.ok_or_else(|| "anonymize requires --input".to_string())?,
        output: output.ok_or_else(|| "anonymize requires --output".to_string())?,
        columns: columns.ok_or_else(|| "anonymize requires --columns".to_string())?,
        deterministic,
        seed,
        force,
    })
}

fn parse_columns(value: &str) -> Result<Vec<usize>, String> {
    let columns = value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            part.trim()
                .parse::<usize>()
                .map_err(|_| format!("invalid column index '{part}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if columns.is_empty() {
        Err("--columns cannot be empty".to_string())
    } else {
        Ok(columns)
    }
}

pub(crate) fn run_cli(action: CliAction) -> Result<(), String> {
    let service = AnonymizerService::new(env!("CARGO_PKG_VERSION"));

    match action {
        CliAction::Analyze { input } => {
            let headers = service
                .analyze_csv(&input)
                .map_err(|error| error.to_string())?;
            println!(
                "CSV Anonymizer {} inspected {} rows in {}",
                service.version(),
                headers.row_count,
                headers.file_path.display()
            );
            for column in headers.columns {
                println!(
                    "{}\t{}\t{:?}\t{:?}",
                    column.index, column.name, column.detected_type, column.pii_risk
                );
            }
            Ok(())
        }
        CliAction::SmokeAnonymize { input, output } => {
            let headers = service
                .analyze_csv(&input)
                .map_err(|error| error.to_string())?;
            let columns = headers
                .columns
                .iter()
                .filter(|column| should_auto_select(column))
                .map(|column| column.index)
                .collect::<Vec<_>>();
            if columns.is_empty() {
                return Err("smoke input did not contain auto-selectable columns".to_string());
            }

            let preview = service
                .preview_anonymization(PreviewParams {
                    file_path: input.clone(),
                    columns: columns.clone(),
                    deterministic: true,
                    seed: "csv-anonymizer-smoke".to_string(),
                    sample_count: 2,
                })
                .map_err(|error| error.to_string())?;
            if preview.previews.is_empty() {
                return Err("smoke preview did not produce any column samples".to_string());
            }

            let result = service
                .anonymize_csv(AnonymizeParams {
                    file_path: input,
                    output_path: output,
                    columns,
                    deterministic: true,
                    seed: "csv-anonymizer-smoke".to_string(),
                    force: true,
                })
                .map_err(|error| error.to_string())?;
            println!(
                "CSV Anonymizer smoke OK: wrote {} rows to {} in {} ms",
                result.row_count,
                result.output_path.display(),
                result.duration_ms
            );
            Ok(())
        }
        CliAction::Anonymize {
            input,
            output,
            columns,
            deterministic,
            seed,
            force,
        } => {
            let result = service
                .anonymize_csv(AnonymizeParams {
                    file_path: input,
                    output_path: output,
                    columns,
                    deterministic,
                    seed,
                    force,
                })
                .map_err(|error| error.to_string())?;
            println!(
                "Wrote {} rows to {} in {} ms",
                result.row_count,
                result.output_path.display(),
                result.duration_ms
            );
            Ok(())
        }
        CliAction::Gui | CliAction::Help | CliAction::Version => Ok(()),
    }
}

pub(crate) fn print_help() {
    println!(
        "CSV Anonymizer {version}

Usage:
  csv-anonymizer
  csv-anonymizer analyze <input.csv>
  csv-anonymizer anonymize --input <input.csv> --output <output.csv> --columns <0,1> [--deterministic] [--seed <seed>] [--force]
  csv-anonymizer --smoke-anonymize <input.csv> <output.csv>

Options:
  --help, -h       Show this help.
  --version, -V    Print the application version.",
        version = env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_smoke_command() {
        assert_eq!(
            parse_cli_args(os_args(&["--smoke-anonymize", "input.csv", "output.csv"])).unwrap(),
            CliAction::SmokeAnonymize {
                input: PathBuf::from("input.csv"),
                output: PathBuf::from("output.csv")
            }
        );
    }

    #[test]
    fn macos_process_serial_arg_starts_gui() {
        assert_eq!(
            parse_cli_args(os_args(&["-psn_0_123"])).unwrap(),
            CliAction::Gui
        );
    }

    #[test]
    fn parses_anonymize_command() {
        assert_eq!(
            parse_cli_args(os_args(&[
                "anonymize",
                "--input",
                "input.csv",
                "--output",
                "output.csv",
                "--columns",
                "1,3",
                "--deterministic",
                "--seed",
                "stable",
                "--force",
            ]))
            .unwrap(),
            CliAction::Anonymize {
                input: PathBuf::from("input.csv"),
                output: PathBuf::from("output.csv"),
                columns: vec![1, 3],
                deterministic: true,
                seed: "stable".to_string(),
                force: true,
            }
        );
    }

    #[test]
    fn rejects_missing_columns() {
        assert!(
            parse_cli_args(os_args(&[
                "anonymize",
                "--input",
                "input.csv",
                "--output",
                "output.csv"
            ]))
            .unwrap_err()
            .contains("--columns")
        );
    }
}
