fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    chopper::exe_runtime::run(&args)
}
