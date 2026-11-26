fn main() {
    std::process::exit(colab_server::cli(&std::env::args().skip(1).collect::<Vec<_>>()))
}
