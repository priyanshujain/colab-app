#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        return colab_app_lib::bridge::run();
    }
    colab_app_lib::run()
}
