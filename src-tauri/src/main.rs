#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => return colab_server::bridge::run(&["server", "run"]),
        Some("server") => std::process::exit(colab_server::cli(&args[1..])),
        _ => {}
    }
    colab_app_lib::run()
}
