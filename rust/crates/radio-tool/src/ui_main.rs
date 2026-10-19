#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let arg = std::env::args_os().nth(1);
    if arg.as_ref().is_some_and(|a| a == "--version" || a == "-V") {
        println!("radio_tool {}", env!("CARGO_PKG_VERSION"));
        return std::process::ExitCode::SUCCESS;
    }
    let open = arg.map(std::path::PathBuf::from).filter(|p| p.is_file());
    radio_tool_app::gui::run(open)
}
