#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let arg = std::env::args_os().nth(1);
    if arg.as_ref().is_some_and(|a| a == "--version" || a == "-V") {
        println!("radio_tool {}", env!("CARGO_PKG_VERSION"));
        return std::process::ExitCode::SUCCESS;
    }
    if arg.as_ref().is_some_and(|a| a == "--render-check") {
        let ok = radio_tool_app::gui::render_check::report();
        return if ok {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }
    let open = arg.map(std::path::PathBuf::from).filter(|p| p.is_file());
    radio_tool_app::gui::run(open)
}
