mod app_logic;
mod cli;
mod settings;
mod theme;

use app_logic::{default_output_path_with_suffix, should_auto_select};
use cli::{CliAction, parse_cli_args, print_help, run_cli};
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerService, HeadersData, PreviewData, PreviewParams,
};
use eframe::egui;
use settings::{AppSettings, SettingsStore};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;
use theme::*;

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
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("CSV Anonymizer")
        .with_app_id("io.github.ddv1982.csv-data-anonymizer")
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([920.0, 640.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "CSV Anonymizer",
        options,
        Box::new(|cc| {
            apply_app_style(&cc.egui_ctx);
            Ok(Box::<CsvAnonymizerApp>::default())
        }),
    )
}

fn app_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../build/icons/512x512.png")).ok()
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

        let available_size = ui.available_size();
        egui::Frame::new()
            .fill(app_background())
            .inner_margin(egui::Margin::symmetric(18, 16))
            .show(ui, |ui| {
                ui.set_min_size(available_size);

                self.render_header(ui);
                ui.add_space(12.0);

                egui::ScrollArea::vertical()
                    .id_salt("main_scroll")
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_section(ui, "Files", |ui| self.render_file_controls(ui));
                        ui.add_space(10.0);

                        render_section(ui, "Settings", |ui| self.render_options(ui));
                        ui.add_space(10.0);

                        render_section(ui, "Detected Columns", |ui| self.render_columns(ui));
                        ui.add_space(10.0);

                        render_section(ui, "Preview", |ui| self.render_preview(ui));
                        ui.add_space(10.0);

                        render_section(ui, "Run", |ui| {
                            self.render_actions(ui);
                            self.render_status(ui);
                        });
                    });
            });

        if self.state.is_anonymizing {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl CsvAnonymizerApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let file_label = self
            .state
            .input_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map_or_else(|| "No CSV loaded".to_string(), ToString::to_string);
        let rows_label = self.state.headers.as_ref().map_or_else(
            || "No rows".to_string(),
            |headers| format!("{} rows", headers.row_count),
        );
        let selected_label = format!("{} selected", self.state.selected_columns.len());
        let mode_label = if self.settings.deterministic_default {
            "Deterministic"
        } else {
            "Randomized"
        };

        egui::Frame::new()
            .fill(header_fill())
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(16, 14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("CSV Anonymizer")
                                .strong()
                                .size(27.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            egui::RichText::new(file_label)
                                .size(13.0)
                                .color(text_muted()),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(secondary_button("Quit")).clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        chip(
                            ui,
                            format!("v{}", self.service.version()),
                            header_chip_fill(),
                            subtle_stroke(),
                            text_primary(),
                        );
                        chip(
                            ui,
                            mode_label,
                            accent_chip_fill(),
                            egui::Stroke::new(1.0, accent()),
                            accent_dark(),
                        );
                        chip(
                            ui,
                            selected_label,
                            muted_chip_fill(),
                            subtle_stroke(),
                            text_primary(),
                        );
                        chip(
                            ui,
                            rows_label,
                            muted_chip_fill(),
                            subtle_stroke(),
                            text_primary(),
                        );
                    });
                });
            });
    }

    fn render_file_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_sized(
                [72.0, 32.0],
                egui::Label::new(egui::RichText::new("Input").strong().color(text_primary())),
            );
            let text_width = (ui.available_width() - 292.0).max(260.0);
            let response = ui.add_sized(
                [text_width, 32.0],
                egui::TextEdit::singleline(&mut self.state.input_path_text)
                    .hint_text("Select or paste a CSV path"),
            );
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                self.load_csv_from_text();
            }

            if ui
                .add_enabled(!self.state.is_anonymizing, secondary_button("Open CSV"))
                .clicked()
                && let Some(path) = self.input_file_dialog().pick_file()
            {
                self.load_csv(path);
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && !self.state.input_path_text.trim().is_empty(),
                    secondary_button("Load Path"),
                )
                .clicked()
            {
                self.load_csv_from_text();
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && self.state.input_path.is_some(),
                    secondary_button("Clear"),
                )
                .clicked()
            {
                self.reset_file_state();
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [72.0, 32.0],
                egui::Label::new(egui::RichText::new("Output").strong().color(text_primary())),
            );
            let text_width = (ui.available_width() - 126.0).max(260.0);
            let output_changed = ui
                .add_sized(
                    [text_width, 32.0],
                    egui::TextEdit::singleline(&mut self.state.output_path_text)
                        .hint_text("Output CSV path"),
                )
                .changed();
            if output_changed {
                self.sync_output_path_from_text();
                self.clear_result();
            }

            if ui
                .add_enabled(
                    !self.state.is_anonymizing && self.state.input_path.is_some(),
                    secondary_button("Choose Folder"),
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
        ui.horizontal_wrapped(|ui| {
            settings_changed |= ui
                .checkbox(&mut self.settings.deterministic_default, "Deterministic")
                .changed();
            settings_changed |= ui
                .checkbox(&mut self.settings.overwrite_output, "Overwrite output")
                .changed();
            settings_changed |= ui
                .checkbox(&mut self.settings.remember_last_paths, "Remember paths")
                .changed();
        });
        ui.add_space(8.0);

        egui::Grid::new("settings_grid")
            .num_columns(6)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Seed").strong().color(text_muted()));
                settings_changed |= ui
                    .add_sized(
                        [260.0, 32.0],
                        egui::TextEdit::singleline(&mut self.settings.seed)
                            .hint_text("Seed for deterministic output"),
                    )
                    .changed();
                ui.label(
                    egui::RichText::new("Output suffix")
                        .strong()
                        .color(text_muted()),
                );
                settings_changed |= ui
                    .add_sized(
                        [140.0, 32.0],
                        egui::TextEdit::singleline(&mut self.settings.default_output_suffix),
                    )
                    .changed();
                ui.end_row();

                ui.label(
                    egui::RichText::new("Sample rows")
                        .strong()
                        .color(text_muted()),
                );
                settings_changed |= ui
                    .add_sized(
                        [86.0, 32.0],
                        egui::DragValue::new(&mut self.settings.sample_row_count)
                            .range(1..=10_000)
                            .speed(1),
                    )
                    .changed();
                ui.label(
                    egui::RichText::new("Preview rows")
                        .strong()
                        .color(text_muted()),
                );
                settings_changed |= ui
                    .add_sized(
                        [86.0, 32.0],
                        egui::DragValue::new(&mut self.settings.preview_sample_count)
                            .range(1..=100)
                            .speed(1),
                    )
                    .changed();
                ui.end_row();
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
            empty_state(
                ui,
                "No CSV selected",
                "Open a CSV to inspect detected columns.",
            );
            return;
        };
        let row_count = headers.row_count;
        let columns = headers.columns.clone();

        ui.horizontal(|ui| {
            chip(
                ui,
                format!("{} rows", row_count),
                subtle_fill(),
                subtle_stroke(),
                text_primary(),
            );
            chip(
                ui,
                format!("{} columns", columns.len()),
                subtle_fill(),
                subtle_stroke(),
                text_primary(),
            );
            chip(
                ui,
                format!("{} selected", self.state.selected_columns.len()),
                accent_chip_fill(),
                egui::Stroke::new(1.0, accent()),
                accent_dark(),
            );
            if ui
                .add_enabled(!self.state.is_anonymizing, secondary_button("Select PII"))
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
                    secondary_button("Clear Selection"),
                )
                .clicked()
            {
                self.state.selected_columns.clear();
                self.state.preview = None;
                self.clear_result();
            }
        });
        ui.add_space(8.0);

        let selected_columns = self.state.selected_columns.clone();
        let mut selection_change = None;

        egui::ScrollArea::vertical()
            .id_salt("columns")
            .max_height(260.0)
            .show(ui, |ui| {
                egui::Grid::new("columns_grid")
                    .striped(true)
                    .num_columns(7)
                    .spacing([12.0, 7.0])
                    .show(ui, |ui| {
                        ui.strong("Use");
                        ui.strong("Index");
                        ui.strong("Name");
                        ui.strong("Type");
                        ui.strong("Confidence");
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
                            ui.add(egui::Label::new(truncate_text(&column.name, 32)).truncate())
                                .on_hover_text(&column.name);
                            ui.label(format_data_type(column.detected_type));
                            confidence_badge(ui, column.confidence);
                            risk_badge(ui, column.pii_risk);
                            let sample_text = sample_summary(&column.sample_values);
                            let response = ui.add(egui::Label::new(sample_text).truncate());
                            if !column.sample_values.is_empty() {
                                response.on_hover_text(column.sample_values.join("\n"));
                            }
                            ui.end_row();
                        }
                    });
            });

        if let Some((index, selected)) = selection_change {
            self.set_column_selected(index, selected);
        }
    }

    fn render_preview(&mut self, ui: &mut egui::Ui) {
        let can_preview = !self.state.is_anonymizing
            && self.state.input_path.is_some()
            && !self.state.selected_columns.is_empty();

        ui.horizontal(|ui| {
            if ui
                .add_enabled(can_preview, secondary_button("Preview"))
                .clicked()
            {
                self.preview();
            }
            chip(
                ui,
                format!("{} columns selected", self.state.selected_columns.len()),
                subtle_fill(),
                subtle_stroke(),
                text_primary(),
            );
        });

        if ui.ctx().input(|input| input.key_pressed(egui::Key::F5)) && can_preview {
            self.preview();
        }

        let Some(preview) = &self.state.preview else {
            if self.state.input_path.is_none() {
                empty_state(
                    ui,
                    "No preview available",
                    "Open a CSV before previewing changes.",
                );
            } else if self.state.selected_columns.is_empty() {
                empty_state(
                    ui,
                    "No columns selected",
                    "Select at least one detected column to preview output.",
                );
            } else {
                empty_state(
                    ui,
                    "Preview not generated",
                    "Run a preview for selected columns.",
                );
            }
            return;
        };

        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("preview")
            .max_height(220.0)
            .show(ui, |ui| {
                for column in &preview.previews {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} ({})",
                                column.column_name, column.column_index
                            ))
                            .strong()
                            .color(text_primary()),
                        );
                        chip(
                            ui,
                            format!("{} samples", column.samples.len()),
                            subtle_fill(),
                            subtle_stroke(),
                            text_muted(),
                        );
                    });
                    egui::Grid::new(format!("preview_{}", column.column_index))
                        .num_columns(3)
                        .striped(true)
                        .spacing([10.0, 5.0])
                        .show(ui, |ui| {
                            for sample in &column.samples {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(truncate_text(&sample.original, 54))
                                            .monospace()
                                            .background_color(subtle_fill()),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&sample.original);
                                ui.label(egui::RichText::new("->").color(text_muted()));
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(truncate_text(&sample.anonymized, 54))
                                            .monospace()
                                            .background_color(subtle_fill()),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(&sample.anonymized);
                                ui.end_row();
                            }
                        });
                    for sample in &column.samples {
                        if sample.original.is_empty() && sample.anonymized.is_empty() {
                            ui.label(
                                egui::RichText::new("Empty value preserved").color(text_muted()),
                            );
                        }
                    }
                    ui.add_space(8.0);
                }
            });
    }

    fn render_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let can_anonymize = !self.state.is_anonymizing
                && self.state.input_path.is_some()
                && !self.state.output_path_text.trim().is_empty()
                && !self.state.selected_columns.is_empty();
            if ui
                .add_enabled(can_anonymize, primary_button("Anonymize CSV"))
                .clicked()
            {
                self.anonymize();
            }

            let hint = if self.state.is_anonymizing {
                "Anonymization is running."
            } else if self.state.input_path.is_none() {
                "Open a CSV to begin."
            } else if self.state.selected_columns.is_empty() {
                "Select at least one column."
            } else if self.state.output_path_text.trim().is_empty() {
                "Choose an output path."
            } else {
                "Ready to anonymize."
            };
            ui.label(egui::RichText::new(hint).color(text_muted()));
        });
    }

    fn render_status(&mut self, ui: &mut egui::Ui) {
        let warning = self.state.settings_warning.clone();
        let error = self.state.error.clone();
        let result = self.state.last_result.clone();

        if self.state.is_anonymizing {
            ui.add_space(8.0);
            status_frame(accent_chip_fill(), accent()).show(ui, |ui| {
                ui.add(egui::Spinner::new());
                ui.label(egui::RichText::new("Anonymizing CSV...").color(accent_highlight()));
            });
        }

        if let Some(warning) = warning {
            ui.add_space(8.0);
            status_frame(warning_fill(), warning_stroke()).show(ui, |ui| {
                ui.label(egui::RichText::new(warning).color(warning_text()));
            });
        }

        if let Some(error) = error {
            ui.add_space(8.0);
            status_frame(danger_fill(), danger_stroke()).show(ui, |ui| {
                ui.label(egui::RichText::new(error).color(danger_text()));
            });
        }

        if let Some(result) = result {
            ui.add_space(8.0);
            status_frame(success_fill(), success_stroke()).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(result).strong().color(success_text()));
                    if self.state.last_output_path.is_some() && ui.button("Open Folder").clicked() {
                        self.open_output_folder();
                    }
                    if ui.button("Anonymize Another File").clicked() {
                        self.reset_file_state();
                    }
                });
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
