include!("src/tauri_command_list.rs");

macro_rules! command_names {
    ($($command:ident),+ $(,)?) => {
        &[$(stringify!($command)),+]
    };
}

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(tauri_command_list!(command_names)),
        ),
    )
    .expect("failed to build Tauri app permissions")
}
