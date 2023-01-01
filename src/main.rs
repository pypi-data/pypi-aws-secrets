mod state;
mod sources;

use anyhow::Result;
use clap::Parser;
use crate::sources::{PyPiSource, Source, SourceType};
use crate::state::State;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {

}

fn main() -> Result<()> {
    let args: Args = Args::parse();

    let mut state = State::load()?;

    let pypi_data = state.data_for_source(SourceType::PyPi);
    let pypi_source = &(PyPiSource::new(pypi_data)?) as &dyn Source;
    let (new_data, packages) = pypi_source.get_new_packages_to_process(100)?;
    state.update_state(SourceType::PyPi, new_data);
    state.save()?;
    Ok(())
}