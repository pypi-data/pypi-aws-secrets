mod aws;
mod scanners;
mod sources;
mod state;

use crate::scanners::{PossiblyMatchedPackage, Scanner};
use crate::sources::{PyPiSource, Source, SourceType};
use crate::state::State;
use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

fn main() -> Result<()> {
    let _args: Args = Args::parse();

    let mut state = State::load()?;

    let pypi_data = state.data_for_source(SourceType::PyPi);
    let pypi_source = &(PyPiSource::new(pypi_data)?) as &dyn Source;
    let (new_data, packages) = pypi_source.get_new_packages_to_process(3)?;

    let scanner = Scanner {};

    for package in packages {
        println!("Matches for {}", package.file_name());
        let download = scanner.download_package(package)?;
        let matches = scanner.quick_check(download)?;
        if let Some(matches) = matches {
            println!("{:?}", matches);
            println!("Found Keys: {:?}", scanner.full_check(matches)?);
        }
    }

    state.update_state(SourceType::PyPi, new_data);
    // state.save()?;
    Ok(())
}
