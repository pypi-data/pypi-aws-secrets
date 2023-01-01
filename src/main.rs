mod aws;
mod reporter;
mod scanners;
mod sources;
mod state;

use crate::aws::check_aws_keys;
use crate::scanners::Scanner;
use crate::sources::{HexPmSource, PyPiSource, RubyGemsSource, Source, SourceType};
use crate::state::State;
use anyhow::Result;
use clap::Parser;
use itertools::Itertools;
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[clap(default_value = "state.json")]
    state: PathBuf,
    #[clap(long, short)]
    no_save: bool,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let mut state = State::load(&args.state)?;

    let query_results: Result<Vec<_>> = [SourceType::PyPi, SourceType::HexPm, SourceType::RubyGems]
        .into_par_iter()
        .map(|s| -> Result<_> {
            let source_data = state.data_for_source(&s);
            let source = s.create_source(source_data).expect("Error creating source");
            let (new_data, packages) = source.get_new_packages_to_process(150)?;
            Ok((s, new_data, packages))
        })
        .collect();
    let query_results = query_results?;
    let (source_data, packages): (Vec<_>, Vec<_>) = query_results
        .into_iter()
        .map(|(t, d, p)| ((t, d), p))
        .unzip();
    let flat_packages: Vec<_> = packages.into_iter().flatten().collect();
    // let new_source_data: Vec<_> = foo.iter().map(|(s, d, _)| (s, d)).collect();
    // let packages: Vec<_> = foo.into_iter().flat_map(|(_, _, p)| p).collect();

    // // let source_data = state.data_for_source(SourceType::PyPi);
    // // let source = &(PyPiSource::new(source_data)?) as &dyn Source;
    // // let source_data = state.data_for_source(SourceType::RubyGems);
    // // let source = &(RubyGemsSource::new(source_data)?) as &dyn Source;
    // let source_data = state.data_for_source(&SourceType::HexPm);
    // let source = &(HexPmSource::new(source_data)?) as &dyn Source;
    // let (new_data, packages) = source.get_new_packages_to_process(150)?;

    let scanner = Scanner {};

    let all_matches: Result<Vec<_>> = flat_packages
        .into_par_iter()
        .flat_map(|package| -> Result<_> {
            let download = scanner.download_package(package)?;
            let name = download.package.name.clone();
            println!("Downloaded {}", name);
            let result = scanner.quick_check(download);
            println!("Ran quick check on {}", name);
            result
        })
        .flatten()
        .map(|matched| {
            println!(
                "running full check on {} - {}",
                matched.downloaded_package.package.name, matched.downloaded_package.package.version
            );
            scanner.full_check(matched)
        })
        .collect();

    let live_keys = check_aws_keys(all_matches?.into_iter().flatten().collect())?;
    println!("Live keys: {:?}", live_keys);

    for (source_type, source_data) in source_data {
        state.update_state(source_type, source_data);
    }

    if !args.no_save {
        state.save(&args.state)?;
    }
    Ok(())
}
