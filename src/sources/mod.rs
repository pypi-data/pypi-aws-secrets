mod pypi;

use crate::state::SourceData;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

pub use pypi::PyPiSource;

#[derive(Deserialize, Serialize, Debug, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum SourceType {
    PyPi,
    RubyGems,
}

#[derive(Debug, Clone)]
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

pub trait Source {
    fn new(data: SourceData) -> Result<Self>
    where
        Self: Sized;
    fn get_new_packages_to_process(
        &self,
        limit: usize,
    ) -> Result<(SourceData, Vec<PackageToProcess>)>;
}
