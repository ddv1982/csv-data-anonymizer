use csv_anonymizer_core::{Confidence, DataType, PiiRisk};
use eframe::egui;

pub(crate) fn apply_app_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(28.0, egui::FontFamily::Proportional),
    );
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(0);
    style.spacing.button_padding = egui::vec2(11.0, 6.0);
    style.spacing.interact_size = egui::vec2(40.0, 32.0);
    style.spacing.text_edit_width = 320.0;

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = app_background();
    visuals.window_fill = app_background();
    visuals.faint_bg_color = subtle_fill();
    visuals.extreme_bg_color = egui::Color32::from_rgb(3, 7, 18);
    visuals.text_edit_bg_color = Some(egui::Color32::from_rgb(15, 23, 42));
    visuals.code_bg_color = egui::Color32::from_rgb(15, 23, 42);
    visuals.warn_fg_color = warning_text();
    visuals.error_fg_color = danger_text();
    visuals.hyperlink_color = accent_highlight();
    visuals.selection.bg_fill = accent();
    visuals.selection.stroke = egui::Stroke::new(1.0, text_primary());
    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.button_frame = true;
    visuals.striped = true;

    visuals.widgets.noninteractive.bg_fill = section_fill();
    visuals.widgets.noninteractive.bg_stroke = subtle_stroke();
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_primary());
    visuals.widgets.inactive.weak_bg_fill = subtle_fill();
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(31, 41, 55);
    visuals.widgets.inactive.bg_stroke = subtle_stroke();
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(30, 64, 105);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(30, 64, 105);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, accent_highlight());
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(37, 99, 235);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(37, 99, 235);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, accent_highlight());
    visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.open = visuals.widgets.hovered;

    style.visuals = visuals;
    ctx.set_global_style(style);
}

pub(crate) fn app_background() -> egui::Color32 {
    egui::Color32::from_rgb(3, 7, 18)
}

pub(crate) fn header_fill() -> egui::Color32 {
    egui::Color32::from_rgb(3, 7, 18)
}

pub(crate) fn section_fill() -> egui::Color32 {
    egui::Color32::from_rgb(17, 24, 39)
}

pub(crate) fn subtle_fill() -> egui::Color32 {
    egui::Color32::from_rgb(15, 23, 42)
}

pub(crate) fn accent() -> egui::Color32 {
    egui::Color32::from_rgb(59, 130, 246)
}

pub(crate) fn accent_dark() -> egui::Color32 {
    egui::Color32::from_rgb(96, 165, 250)
}

pub(crate) fn accent_highlight() -> egui::Color32 {
    egui::Color32::from_rgb(147, 197, 253)
}

pub(crate) fn text_primary() -> egui::Color32 {
    egui::Color32::from_rgb(243, 244, 246)
}

pub(crate) fn text_muted() -> egui::Color32 {
    egui::Color32::from_rgb(156, 163, 175)
}

pub(crate) fn border_color() -> egui::Color32 {
    egui::Color32::from_rgb(55, 65, 81)
}

pub(crate) fn subtle_stroke() -> egui::Stroke {
    egui::Stroke::new(1.0, border_color())
}

pub(crate) fn muted_chip_fill() -> egui::Color32 {
    egui::Color32::from_rgb(31, 41, 55)
}

pub(crate) fn accent_chip_fill() -> egui::Color32 {
    egui::Color32::from_rgb(30, 64, 105)
}

pub(crate) fn header_chip_fill() -> egui::Color32 {
    egui::Color32::from_rgb(17, 24, 39)
}

pub(crate) fn success_fill() -> egui::Color32 {
    egui::Color32::from_rgb(6, 78, 59)
}

pub(crate) fn success_stroke() -> egui::Color32 {
    egui::Color32::from_rgb(52, 211, 153)
}

pub(crate) fn success_text() -> egui::Color32 {
    egui::Color32::from_rgb(167, 243, 208)
}

pub(crate) fn warning_fill() -> egui::Color32 {
    egui::Color32::from_rgb(69, 45, 15)
}

pub(crate) fn warning_stroke() -> egui::Color32 {
    egui::Color32::from_rgb(245, 158, 11)
}

pub(crate) fn warning_text() -> egui::Color32 {
    egui::Color32::from_rgb(253, 230, 138)
}

pub(crate) fn danger_fill() -> egui::Color32 {
    egui::Color32::from_rgb(69, 10, 10)
}

pub(crate) fn danger_stroke() -> egui::Color32 {
    egui::Color32::from_rgb(248, 113, 113)
}

pub(crate) fn danger_text() -> egui::Color32 {
    egui::Color32::from_rgb(254, 202, 202)
}

fn section_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(section_fill())
        .stroke(subtle_stroke())
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
}

pub(crate) fn render_section(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    section_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(15.0)
                .color(text_primary()),
        );
        ui.add_space(8.0);
        add_contents(ui);
    });
}

pub(crate) fn primary_button(label: &'static str) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label)
            .strong()
            .color(egui::Color32::WHITE),
    )
    .fill(accent())
    .stroke(egui::Stroke::new(1.0, accent_highlight()))
    .corner_radius(6)
    .min_size(egui::vec2(132.0, 34.0))
}

pub(crate) fn secondary_button(label: &'static str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(label).color(text_primary()))
        .fill(muted_chip_fill())
        .stroke(subtle_stroke())
        .corner_radius(6)
        .min_size(egui::vec2(86.0, 32.0))
}

pub(crate) fn chip(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    fill: egui::Color32,
    stroke: egui::Stroke,
    text_color: egui::Color32,
) {
    let text = text.into();
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().strong().color(text_color));
        });
}

pub(crate) fn empty_state(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.add_space(18.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(15.0)
                .color(text_primary()),
        );
        ui.label(egui::RichText::new(detail).color(text_muted()));
    });
    ui.add_space(18.0);
}

pub(crate) fn status_frame(fill: egui::Color32, stroke: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
}

pub(crate) fn format_data_type(data_type: DataType) -> &'static str {
    match data_type {
        DataType::Email => "Email",
        DataType::Uuid => "UUID",
        DataType::Timestamp => "Timestamp",
        DataType::NumericId => "Numeric ID",
        DataType::CountryCode => "Country",
        DataType::Phone => "Phone",
        DataType::FirstName => "First name",
        DataType::LastName => "Last name",
        DataType::FullName => "Full name",
        DataType::Enum => "Enum",
        DataType::String => "String",
        DataType::Unknown => "Unknown",
    }
}

pub(crate) fn confidence_badge(ui: &mut egui::Ui, confidence: Confidence) {
    let (label, fill, stroke, text_color) = match confidence {
        Confidence::High => ("High", success_fill(), success_stroke(), success_text()),
        Confidence::Medium => ("Medium", warning_fill(), warning_stroke(), warning_text()),
        Confidence::Low => ("Low", subtle_fill(), border_color(), text_muted()),
    };

    chip(ui, label, fill, egui::Stroke::new(1.0, stroke), text_color);
}

pub(crate) fn risk_badge(ui: &mut egui::Ui, risk: PiiRisk) {
    let (label, fill, stroke, text_color) = match risk {
        PiiRisk::High => ("High", danger_fill(), danger_stroke(), danger_text()),
        PiiRisk::Medium => ("Medium", warning_fill(), warning_stroke(), warning_text()),
        PiiRisk::Low => ("Low", success_fill(), success_stroke(), success_text()),
    };

    chip(ui, label, fill, egui::Stroke::new(1.0, stroke), text_color);
}

pub(crate) fn sample_summary(values: &[String]) -> String {
    if values.is_empty() {
        return "No samples".to_string();
    }

    let mut samples = values
        .iter()
        .take(3)
        .map(|value| truncate_text(value, 30))
        .collect::<Vec<_>>();
    if values.len() > 3 {
        samples.push("...".to_string());
    }
    samples.join(", ")
}

pub(crate) fn truncate_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return value.chars().take(max_chars).collect();
    }

    let mut truncated = value.chars().take(max_chars - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}
