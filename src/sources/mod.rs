mod pypi;

use crate::state::SourceData;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use url::Url;

pub use pypi::PyPiSource;

#[derive(Deserialize, Serialize, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum SourceType {
    PyPi,
    RubyGems,
}

pub struct PackageToProcess {
    download_url: Url,
    name: String,
    version: String,
}

impl PackageToProcess {
    pub fn new(name: String, version: String, download_url: Url) -> Self {
        Self {
            download_url,
            name,
            version,
        }
    }
}

pub trait Source {
    fn new(data: SourceData) -> Result<Self> where Self: Sized;
    fn get_new_packages_to_process(&self, limit: usize) -> Result<(SourceData, Vec<PackageToProcess>)>;
}
