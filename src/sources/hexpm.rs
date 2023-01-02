use crate::sources::{PackageToProcess, Source, SourceStats, SourceType};
use crate::state::SourceData;
use anyhow::anyhow;
use anyhow::Result;
use chrono::prelude::*;

use chrono_humanize::HumanTime;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::fmt::{Display, Formatter};

#[derive(Serialize, Deserialize)]
pub struct HexPmSource {
    last_package_timestamp: DateTime<Utc>,
    #[serde(default)]
    stats: SourceStats,
}

#[derive(Deserialize, Debug)]
pub struct HexPmResponse {
    name: String,
    releases: Vec<HexPmRelease>,
}

#[derive(Deserialize, Debug)]
pub struct HexPmRelease {
    version: String,
    inserted_at: DateTime<Utc>,
}

impl Display for HexPmSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HexPm - Packages updated from {} ({})",
            self.last_package_timestamp,
            HumanTime::from(self.last_package_timestamp - Utc::now())
        )
    }
}

impl Source for HexPmSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                last_package_timestamp: "2018-01-01T00:00:00Z".parse().unwrap(),
                stats: Default::default(),
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(&mut self, limit: usize) -> Result<Vec<PackageToProcess>> {
        let base_url = "https://hex.pm/api/packages?sort=updated_at&search=".to_string();
        let mut results = vec![];
        let client = reqwest::blocking::Client::new();

        // Hex pages start at 1
        for page in 1..20 {
            if results.len() >= limit {
                break;
            }
            let url = format!("{}&page={}", base_url, page);
            // ToDo: Replace the user agent with a link to the repo
            let response = client
                .get(url)
                .header("User-Agent", "aws-key-finder")
                .send()?;
            let hex_response: Vec<HexPmResponse> = response.json()?;

            if hex_response.is_empty() {
                break;
            } else {
                results.extend(hex_response.into_iter().flat_map(|v| {
                    let new_releases = v
                        .releases
                        .into_iter()
                        .filter(|r| r.inserted_at >= self.last_package_timestamp);
                    new_releases
                        .map(|r| (v.name.clone(), r))
                        .collect::<Vec<_>>()
                }));
            }
        }

        let results: Vec<_> = results.into_iter().rev().take(limit).collect();

        let last_updated_at = results
            .iter()
            .map(|(_, r)| r.inserted_at)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No gem releases found"))?;

        self.last_package_timestamp = last_updated_at;

        let to_process = results
            .into_iter()
            .map(|(name, release)| {
                PackageToProcess {
                    // https://repo.hex.pm/tarballs/mathlogic_s3_test-0.1.0.tar
                    download_url: format!(
                        "https://repo.hex.pm/tarballs/{}-{}.tar",
                        name, release.version
                    )
                    .parse()
                    .unwrap(),
                    name,
                    version: release.version,
                    source: SourceType::HexPm,
                }
            })
            .collect();
        Ok(to_process)
    }

    fn to_state(&self) -> Result<SourceData> {
        Ok(serde_json::to_value(&self)?)
    }

    fn get_stats(&mut self) -> &mut SourceStats {
        &mut self.stats
    }
}
