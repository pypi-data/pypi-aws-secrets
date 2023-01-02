mod hexpm;
mod pypi;
mod rubygems;

use crate::state::SourceData;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;

use url::Url;

pub use hexpm::HexPmSource;
pub use pypi::PyPiSource;
pub use rubygems::RubyGemsSource;

#[derive(
    Deserialize, Serialize, Debug, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, clap::ValueEnum,
)]
#[clap(rename_all = "lower")]
pub enum SourceType {
    PyPi,
    RubyGems,
    HexPm,
}

impl SourceType {
    pub fn create_source(&self, data: SourceData) -> Result<Box<dyn Source>> {
        Ok(match self {
            SourceType::PyPi => Box::new(PyPiSource::new(data)?),
            SourceType::RubyGems => Box::new(RubyGemsSource::new(data)?),
            SourceType::HexPm => Box::new(HexPmSource::new(data)?),
        })
    }

    pub fn report_path(&self) -> PathBuf {
        let root = PathBuf::from("keys");
        match self {
            SourceType::PyPi => root.join("pypi"),
            SourceType::RubyGems => root.join("rubygems"),
            SourceType::HexPm => root.join("elixir"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct PackageToProcess {
    pub download_url: Url,
    pub name: String,
    pub version: String,
    pub source: SourceType,
}

impl PackageToProcess {
    pub fn new(name: String, version: String, download_url: Url, source: SourceType) -> Self {
        Self {
            download_url,
            name,
            version,
            source,
        }
    }

    pub fn file_name(&self) -> &str {
        self.download_url
            .path_segments()
            .expect("PackageToProcess empty path segments")
            .last()
            .unwrap()
    }
}

pub trait Source: Send + Display {
    fn new(data: SourceData) -> Result<Self>
    where
        Self: Sized;
    fn get_new_packages_to_process(&mut self, limit: usize) -> Result<Vec<PackageToProcess>>;

    fn to_state(&self) -> Result<SourceData>;

    fn get_stats(&mut self) -> &mut SourceStats;
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct SourceStats {
    packages_searched: u64,
}

impl SourceStats {
    pub fn add_packages_searched(&mut self, count: u64) {
        self.packages_searched += count;
    }
}
