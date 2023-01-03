mod aws;
mod reporter;
mod scanners;
mod sources;
mod state;

use crate::aws::check_aws_keys;
use crate::scanners::Scanner;
use crate::sources::SourceType;
use crate::state::State;
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use crate::reporter::create_findings;
use itertools::Itertools;
use rayon::prelude::*;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Run {
        #[clap(default_value = "state.json")]
        state: PathBuf,
        #[clap(long, short)]
        save: bool,
        #[clap(long, short)]
        limit: usize,
        #[arg(value_enum)]
        #[clap(
            long,
            use_value_delimiter = true,
            value_delimiter = ',',
            required = true
        )]
        sources: Vec<SourceType>,
    },
    SetupState {
        #[clap(default_value = "state.json")]
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    match args.action {
        Action::Run {
            state,
            save,
            limit,
            sources,
        } => run(state, save, limit, sources),
        Action::SetupState { path } => {
            let mut state = State::load(&path)?;
            for source_type in SourceType::value_variants() {
                let source_data = state.data_for_source(source_type);
                let source = source_type
                    .create_source(source_data)
                    .expect("Error creating source");
                let serialized_data = source.to_state()?;
                state.update_state(source_type.clone(), serialized_data);
            }
            state.save(&path)?;
            Ok(())
        }
    }
}

fn run(state_path: PathBuf, save: bool, limit: usize, sources: Vec<SourceType>) -> Result<()> {
    let mut state = State::load(&state_path)?;
    let sources: Vec<_> = sources.into_iter().unique().collect();

    let query_results: Result<Vec<_>> = sources
        .into_par_iter()
        .map(|s| -> Result<_> {
            let source_data = state.data_for_source(&s);
            let mut source = s.create_source(source_data).expect("Error creating source");
            println!("Fetching data for source {}", source);
            let packages = source
                .get_new_packages_to_process(limit)
                .with_context(|| format!("Failed to get packages to process for source {:?}", s))?;
            println!("Source {:?} found {} packages", s, packages.len());
            let stats = source.get_stats();
            stats.add_packages_searched(packages.len() as u64);
            Ok((s, source, packages))
        })
        .collect();
    let query_results = query_results?;
    let (source_data, packages): (Vec<_>, Vec<_>) = query_results
        .into_iter()
        .map(|(t, d, p)| ((t, d), p))
        .unzip();
    let flat_packages: Vec<_> = packages.into_iter().flatten().collect();

    let scanner = Scanner {};

    let all_matches: Result<Vec<_>> = flat_packages
        .into_par_iter()
        .flat_map(|package| -> Result<_> {
            let download = scanner.download_package(&package).with_context(|| {
                format!(
                    "Failed to download package {:?} / {} @ {}",
                    package.source, package.name, package.version
                )
            })?;
            let name = download.package.name.clone();
            let version = download.package.version.clone();
            let source = download.package.source.clone();
            let result = scanner.quick_check(download).with_context(|| {
                format!(
                    "Error running quick check on {:?} / {} @ {}",
                    source, name, version
                )
            });
            println!(
                "Finished quick check on {:?} / {} @ {}",
                source, name, version
            );
            result
        })
        .flatten()
        .map(|matched| {
            println!(
                "running full check on {:?} / {} @ {}\n - Previous match:\n{}",
                matched.downloaded_package.package.source,
                matched.downloaded_package.package.name,
                matched.downloaded_package.package.version,
                matched
                    .matches
                    .iter()
                    .map(|v| { v.lines.chars().take(250).join("") })
                    .join("\n\n")
            );
            scanner.full_check(matched)
        })
        .collect();

    let live_keys = check_aws_keys(all_matches?.into_iter().flatten().collect()).context("Error checking AWS keys")?;
    println!("Live keys: {:?}", live_keys);

    create_findings(live_keys)?;

    if save {
        for (source_type, source_data) in source_data {
            state.update_state(source_type, source_data.to_state()?);
        }
        state.save(&state_path)?;
    }

    Ok(())
}
