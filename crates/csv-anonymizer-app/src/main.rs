mod settings;

use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerService, ColumnMetadata, HeadersData, PreviewData,
    PreviewParams,
};
use eframe::egui;
use settings::{AppSettings, SettingsStore};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    match parse_cli_args(std::env::args_os().skip(1))? {
        CliAction::Gui => run_gui().map_err(|error| error.to_string()),
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

fn run_gui() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CSV Anonymizer")
            .with_inner_size([1180.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CSV Anonymizer",
        options,
        Box::new(|_cc| Ok(Box::<CsvAnonymizerApp>::default())),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
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

fn parse_cli_args(args: impl IntoIterator<Item = OsString>) -> Result<CliAction, String> {
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

fn run_cli(action: CliAction) -> Result<(), String> {
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

fn print_help() {
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

struct CsvAnonymizerApp {
    service: AnonymizerService,
    settings_store: SettingsStore,
    settings: AppSettings,
    state: AppState,
}

impl Default for CsvAnonymizerApp {
    fn default() -> Self {
        let settings_store = SettingsStore::default();
        let (settings, settings_warning) = match settings_store.load() {
            Ok(settings) => (settings, None),
            Err(error) => (
                AppSettings::default(),
                Some(format!(
                    "Could not load settings from {}: {error}",
                    settings_store.path().display()
                )),
            ),
        };

        Self {
            service: AnonymizerService::new(env!("CARGO_PKG_VERSION")),
            settings_store,
            settings,
            state: AppState {
                settings_warning,
                ..AppState::default()
            },
        }
    }
}

#[derive(Default)]
struct AppState {
    input_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    input_path_text: String,
    output_path_text: String,
    headers: Option<HeadersData>,
    selected_columns: Vec<usize>,
    preview: Option<PreviewData>,
    pending_anonymize: Option<Receiver<Result<AnonymizeData, String>>>,
    is_anonymizing: bool,
    last_result: Option<String>,
    last_output_path: Option<PathBuf>,
    error: Option<String>,
    settings_warning: Option<String>,
}

impl eframe::App for CsvAnonymizerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_anonymize(&ctx);

        if ui
            .ctx()
            .input(|input| input.modifiers.command && input.key_pressed(egui::Key::Q))
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }

        ui.horizontal(|ui| {
            ui.heading("CSV Anonymizer");
            ui.separator();
            ui.label(format!("v{}", self.service.version()));
        });

        self.render_file_controls(ui);
        ui.separator();
        self.render_options(ui);
        ui.separator();
        self.render_columns(ui);
        ui.separator();
        self.render_preview(ui);
        ui.separator();
        self.render_actions(ui);
        self.render_status(ui);

        if self.state.is_anonymizing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl CsvAnonymizerApp {
    fn render_file_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Input");
            let response = ui.text_edit_singleline(&mut self.state.input_path_text);
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                self.load_csv_from_text();
            }

            if ui
                .add_enabled(!self.state.is_anonymizing, egui::Button::new("Open CSV"))
                .clicked()
                && let Some(path) = self.input_file_dialog().pick_file()
            {
                self.load_csv(path);
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && !self.state.input_path_text.trim().is_empty(),
                    egui::Button::new("Load Path"),
                )
                .clicked()
            {
                self.load_csv_from_text();
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && self.state.input_path.is_some(),
                    egui::Button::new("Clear"),
                )
                .clicked()
            {
                self.reset_file_state();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Output");
            let output_changed = ui
                .text_edit_singleline(&mut self.state.output_path_text)
                .changed();
            if output_changed {
                self.sync_output_path_from_text();
                self.clear_result();
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && self.state.input_path.is_some(),
                    egui::Button::new("Choose Folder"),
                )
                .clicked()
                && let Some(folder) = self.output_folder_dialog().pick_folder()
            {
                let file_name = self
                    .state
                    .output_path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("anonymized.csv"));
                let output_path = folder.join(file_name);
                self.set_output_path(output_path);
                self.remember_output_directory(&folder);
                self.clear_result();
            }
        });
    }

    fn render_options(&mut self, ui: &mut egui::Ui) {
        let mut settings_changed = false;
        ui.horizontal(|ui| {
            settings_changed |= ui
                .checkbox(&mut self.settings.deterministic_default, "Deterministic")
                .changed();
            settings_changed |= ui
                .checkbox(&mut self.settings.overwrite_output, "Overwrite output")
                .changed();
            settings_changed |= ui
                .checkbox(&mut self.settings.remember_last_paths, "Remember paths")
                .changed();
            ui.label("Seed");
            settings_changed |= ui.text_edit_singleline(&mut self.settings.seed).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Sample rows");
            settings_changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings.sample_row_count)
                        .range(1..=10_000)
                        .speed(1),
                )
                .changed();
            ui.label("Preview rows");
            settings_changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings.preview_sample_count)
                        .range(1..=100)
                        .speed(1),
                )
                .changed();
            ui.label("Output suffix");
            settings_changed |= ui
                .text_edit_singleline(&mut self.settings.default_output_suffix)
                .changed();
        });

        if settings_changed {
            if !self.settings.remember_last_paths {
                self.settings.last_input_directory = None;
                self.settings.last_output_directory = None;
            }
            self.save_settings();
        }
    }

    fn render_columns(&mut self, ui: &mut egui::Ui) {
        let Some(headers) = &self.state.headers else {
            ui.label("Open a CSV to inspect columns.");
            return;
        };
        let row_count = headers.row_count;
        let columns = headers.columns.clone();

        ui.horizontal(|ui| {
            ui.label(format!("Rows: {}", row_count));
            if ui
                .add_enabled(!self.state.is_anonymizing, egui::Button::new("Select PII"))
                .clicked()
            {
                self.state.selected_columns = columns
                    .iter()
                    .filter(|column| should_auto_select(column))
                    .map(|column| column.index)
                    .collect();
                self.state.preview = None;
                self.clear_result();
            }
            if ui
                .add_enabled(
                    !self.state.is_anonymizing,
                    egui::Button::new("Clear Selection"),
                )
                .clicked()
            {
                self.state.selected_columns.clear();
                self.state.preview = None;
                self.clear_result();
            }
        });

        let selected_columns = self.state.selected_columns.clone();
        let mut selection_change = None;

        egui::ScrollArea::vertical()
            .id_salt("columns")
            .max_height(220.0)
            .show(ui, |ui| {
                egui::Grid::new("columns_grid")
                    .striped(true)
                    .num_columns(6)
                    .show(ui, |ui| {
                        ui.strong("Use");
                        ui.strong("Index");
                        ui.strong("Name");
                        ui.strong("Type");
                        ui.strong("PII");
                        ui.strong("Samples");
                        ui.end_row();

                        for column in columns {
                            let mut selected = selected_columns.contains(&column.index);
                            if ui
                                .add_enabled(
                                    !self.state.is_anonymizing,
                                    egui::Checkbox::new(&mut selected, ""),
                                )
                                .changed()
                            {
                                selection_change = Some((column.index, selected));
                            }
                            ui.label(column.index.to_string());
                            ui.label(&column.name);
                            ui.label(format!("{:?}", column.detected_type));
                            ui.label(format!("{:?}", column.pii_risk));
                            ui.label(column.sample_values.join(", "));
                            ui.end_row();
                        }
                    });
            });

        if let Some((index, selected)) = selection_change {
            self.set_column_selected(index, selected);
        }
    }

    fn render_preview(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                !self.state.is_anonymizing
                    && self.state.input_path.is_some()
                    && !self.state.selected_columns.is_empty(),
                egui::Button::new("Preview"),
            )
            .clicked()
        {
            self.preview();
        }

        let Some(preview) = &self.state.preview else {
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt("preview")
            .max_height(180.0)
            .show(ui, |ui| {
                for column in &preview.previews {
                    ui.strong(format!("{} ({})", column.column_name, column.column_index));
                    for sample in &column.samples {
                        ui.horizontal_wrapped(|ui| {
                            ui.monospace(&sample.original);
                            ui.label("->");
                            ui.monospace(&sample.anonymized);
                        });
                    }
                    ui.add_space(8.0);
                }
            });
    }

    fn render_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.state.is_anonymizing
                        && self.state.input_path.is_some()
                        && !self.state.output_path_text.trim().is_empty()
                        && !self.state.selected_columns.is_empty(),
                    egui::Button::new("Anonymize CSV"),
                )
                .clicked()
            {
                self.anonymize();
            }

            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn render_status(&mut self, ui: &mut egui::Ui) {
        if self.state.is_anonymizing {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label("Anonymizing CSV...");
            });
        }

        if let Some(warning) = &self.state.settings_warning {
            ui.colored_label(egui::Color32::from_rgb(145, 95, 20), warning);
        }

        if let Some(error) = &self.state.error {
            ui.colored_label(egui::Color32::from_rgb(180, 30, 30), error);
        }

        if let Some(result) = self.state.last_result.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(egui::Color32::from_rgb(30, 110, 45), result);
                if self.state.last_output_path.is_some() && ui.button("Open Folder").clicked() {
                    self.open_output_folder();
                }
                if ui.button("Anonymize Another File").clicked() {
                    self.reset_file_state();
                }
            });
        }
    }

    fn input_file_dialog(&self) -> rfd::FileDialog {
        let mut dialog = rfd::FileDialog::new().add_filter("CSV", &["csv"]);
        if self.settings.remember_last_paths
            && let Some(directory) = self
                .settings
                .last_input_directory
                .as_ref()
                .filter(|path| path.is_dir())
        {
            dialog = dialog.set_directory(directory);
        }
        dialog
    }

    fn output_folder_dialog(&self) -> rfd::FileDialog {
        let mut dialog = rfd::FileDialog::new();
        if self.settings.remember_last_paths
            && let Some(directory) = self
                .settings
                .last_output_directory
                .as_ref()
                .filter(|path| path.is_dir())
        {
            dialog = dialog.set_directory(directory);
        }
        dialog
    }

    fn load_csv_from_text(&mut self) {
        let path = self.state.input_path_text.trim();
        if path.is_empty() {
            self.state.error = Some("Enter an input CSV path.".to_string());
            return;
        }
        self.load_csv(PathBuf::from(path));
    }

    fn load_csv(&mut self, path: PathBuf) {
        self.clear_result();
        match self
            .service
            .analyze_csv_with_sample_rows(&path, self.settings.sample_row_count)
        {
            Ok(headers) => {
                let output_path =
                    self.suggest_output_path(&path, headers.default_output_path.clone());
                self.state.output_path = Some(output_path.clone());
                self.state.output_path_text = output_path.display().to_string();
                self.state.selected_columns = headers
                    .columns
                    .iter()
                    .filter(|column| should_auto_select(column))
                    .map(|column| column.index)
                    .collect();
                self.state.input_path_text = path.display().to_string();
                self.state.input_path = Some(path.clone());
                self.state.headers = Some(headers);
                self.state.preview = None;
                self.remember_input_directory(&path);
                if !self.state.selected_columns.is_empty() {
                    self.preview();
                }
            }
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }

    fn suggest_output_path(&self, input_path: &Path, fallback: PathBuf) -> PathBuf {
        let mut output_path =
            default_output_path_with_suffix(input_path, self.settings.default_output_suffix.trim());
        if self.settings.remember_last_paths
            && let Some(directory) = self
                .settings
                .last_output_directory
                .as_ref()
                .filter(|path| path.is_dir())
            && let Some(file_name) = output_path.file_name()
        {
            output_path = directory.join(file_name);
        }

        if output_path.as_os_str().is_empty() {
            fallback
        } else {
            output_path
        }
    }

    fn preview(&mut self) {
        self.clear_result();
        let Some(input_path) = self.state.input_path.clone() else {
            return;
        };

        match self.service.preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: self.state.selected_columns.clone(),
            deterministic: self.settings.deterministic_default,
            seed: self.settings.seed.clone(),
            sample_count: self.settings.preview_sample_count,
        }) {
            Ok(preview) => self.state.preview = Some(preview),
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }

    fn anonymize(&mut self) {
        self.clear_result();
        self.sync_output_path_from_text();
        let Some(input_path) = self.state.input_path.clone() else {
            self.state.error = Some("Open an input CSV before anonymizing.".to_string());
            return;
        };
        let Some(output_path) = self.state.output_path.clone() else {
            self.state.error = Some("Choose or enter an output CSV path.".to_string());
            return;
        };
        if self.state.selected_columns.is_empty() {
            self.state.error = Some("Select at least one column to anonymize.".to_string());
            return;
        }

        if let Some(parent) = output_path.parent() {
            self.remember_output_directory(parent);
        }

        let service = self.service.clone();
        let sample_row_count = self.settings.sample_row_count;
        let params = AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: self.state.selected_columns.clone(),
            deterministic: self.settings.deterministic_default,
            seed: self.settings.seed.clone(),
            force: self.settings.overwrite_output,
        };
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let result = service
                .anonymize_csv_with_sample_rows(params, sample_row_count)
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
        });

        self.state.pending_anonymize = Some(receiver);
        self.state.is_anonymizing = true;
    }

    fn poll_anonymize(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.state.pending_anonymize.as_ref() else {
            return;
        };

        let outcome = match receiver.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                None
            }
            Err(TryRecvError::Disconnected) => Some(Err(
                "Anonymization stopped before returning a result.".to_string(),
            )),
        };

        if let Some(outcome) = outcome {
            self.state.pending_anonymize = None;
            self.state.is_anonymizing = false;
            match outcome {
                Ok(result) => {
                    self.state.last_output_path = Some(result.output_path.clone());
                    self.state.last_result = Some(format!(
                        "Wrote {} rows to {} in {} ms",
                        result.row_count,
                        result.output_path.display(),
                        result.duration_ms
                    ));
                }
                Err(error) => self.state.error = Some(error),
            }
            ctx.request_repaint();
        }
    }

    fn set_column_selected(&mut self, index: usize, selected: bool) {
        if selected {
            if !self.state.selected_columns.contains(&index) {
                self.state.selected_columns.push(index);
                self.state.selected_columns.sort_unstable();
            }
        } else {
            self.state
                .selected_columns
                .retain(|selected| *selected != index);
        }
        self.state.preview = None;
        self.clear_result();
    }

    fn set_output_path(&mut self, output_path: PathBuf) {
        self.state.output_path_text = output_path.display().to_string();
        self.state.output_path = Some(output_path);
    }

    fn sync_output_path_from_text(&mut self) {
        let text = self.state.output_path_text.trim();
        self.state.output_path = if text.is_empty() {
            None
        } else {
            Some(PathBuf::from(text))
        };
    }

    fn remember_input_directory(&mut self, input_path: &Path) {
        if !self.settings.remember_last_paths {
            return;
        }
        if let Some(parent) = input_path.parent() {
            self.settings.last_input_directory = Some(parent.to_path_buf());
            self.save_settings();
        }
    }

    fn remember_output_directory(&mut self, output_directory: &Path) {
        if !self.settings.remember_last_paths {
            return;
        }
        self.settings.last_output_directory = Some(output_directory.to_path_buf());
        self.save_settings();
    }

    fn save_settings(&mut self) {
        match self.settings_store.save(&self.settings) {
            Ok(()) => self.state.settings_warning = None,
            Err(error) => {
                self.state.settings_warning = Some(format!(
                    "Could not save settings to {}: {error}",
                    self.settings_store.path().display()
                ));
            }
        }
    }

    fn open_output_folder(&mut self) {
        let Some(output_path) = self.state.last_output_path.clone() else {
            return;
        };
        let target = output_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(output_path);
        if let Err(error) = open::that_detached(&target) {
            self.state.error = Some(format!("Could not open {}: {error}", target.display()));
        }
    }

    fn reset_file_state(&mut self) {
        self.state.input_path = None;
        self.state.output_path = None;
        self.state.input_path_text.clear();
        self.state.output_path_text.clear();
        self.state.headers = None;
        self.state.selected_columns.clear();
        self.state.preview = None;
        self.state.last_result = None;
        self.state.last_output_path = None;
        self.clear_result();
    }

    fn clear_result(&mut self) {
        self.state.error = None;
        self.state.last_result = None;
    }
}

fn default_output_path_with_suffix(input_path: &Path, suffix: &str) -> PathBuf {
    let suffix = if suffix.trim().is_empty() {
        "_anonymized"
    } else {
        suffix.trim()
    };
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = match input_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{stem}{suffix}.{extension}"),
        _ => format!("{stem}{suffix}"),
    };
    input_path.with_file_name(file_name)
}

fn should_auto_select(column: &ColumnMetadata) -> bool {
    !column.sample_values.is_empty()
        && matches!(
            column.pii_risk,
            csv_anonymizer_core::PiiRisk::High | csv_anonymizer_core::PiiRisk::Medium
        )
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

    #[test]
    fn builds_output_path_with_custom_suffix() {
        assert_eq!(
            default_output_path_with_suffix(Path::new("/tmp/data.csv"), "_private"),
            PathBuf::from("/tmp/data_private.csv")
        );
    }
}
