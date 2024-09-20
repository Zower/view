fn main() -> miette::Result<()> {
    // let mut command = std::process::Command::new("cargo");

    // command.arg("watch").arg("-x").arg("run");

    // command.spawn().expect("Spawn to work");

    paladin_view::surrogate::run()
}
