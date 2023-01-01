use crate::sources::{PackageToProcess, Source, SourceType};
use crate::state::SourceData;
use anyhow::anyhow;
use anyhow::Result;
use chrono::prelude::*;

use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize)]
pub struct HexPmSource {
    last_updated_at: DateTime<Utc>,
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

impl Source for HexPmSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                last_updated_at: "2018-01-01T00:00:00Z".parse().unwrap(),
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(
        &self,
        limit: usize,
    ) -> anyhow::Result<(SourceData, Vec<PackageToProcess>)> {
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
                        .filter(|r| r.inserted_at >= self.last_updated_at);
                    new_releases
                        .map(|r| (v.name.clone(), r))
                        .collect::<Vec<_>>()
                }));
            }
        }

        let results: Vec<_> = results.into_iter().take(limit).collect();

        let last_updated_at = results
            .iter()
            .map(|(_, r)| r.inserted_at)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No gem releases found"))?;

        let new_state = serde_json::to_value(HexPmSource { last_updated_at })?;

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
        Ok((new_state, to_process))
    }
}
