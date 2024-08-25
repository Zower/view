use std::path::PathBuf;

use components::root::Root;

use miette::IntoDiagnostic;
use view::prelude::*;
mod components;

fn main() -> view::Result<()> {
    run(Root)
}

pub struct InitResult {
    pub workspace: PathBuf,
    pub file: Option<PathBuf>,
}

pub fn initial_workspace() -> miette::Result<InitResult> {
    let workspace = PathBuf::from("./").canonicalize().into_diagnostic()?;

    let mut args = std::env::args();
    let _ = args.next();

    let file = args.next();

    Ok(InitResult {
        workspace,
        file: file.map(Into::into),
    })
}
