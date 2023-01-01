mod aws;
mod scanners;
mod sources;
mod state;

use crate::scanners::{Scanner};
use crate::sources::{PyPiSource, Source, SourceType};
use crate::state::State;
use anyhow::Result;
use clap::Parser;
use rayon::prelude::*;
use crate::aws::check_aws_keys;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

fn main() -> Result<()> {
    let _args: Args = Args::parse();

    let mut state = State::load()?;

    let pypi_data = state.data_for_source(SourceType::PyPi);
    let pypi_source = &(PyPiSource::new(pypi_data)?) as &dyn Source;
    let (new_data, packages) = pypi_source.get_new_packages_to_process(150)?;

    let scanner = Scanner {};

    let all_matches: Result<Vec<_>> = packages.into_par_iter().flat_map(|package| -> Result<_> {
        println!("Matches for {}", package.file_name());
        let download = scanner.download_package(package)?;
        scanner.quick_check(download)
    }).flatten().map(|matches| {
        scanner.full_check(matches)
    }).collect();

    let live_keys = check_aws_keys(all_matches?.into_iter().flatten().collect())?;
    println!("Live keys: {:?}", live_keys);

    state.update_state(SourceType::PyPi, new_data);
    state.save()?;
    Ok(())
}
