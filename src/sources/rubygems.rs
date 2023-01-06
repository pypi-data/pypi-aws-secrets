use crate::sources::{PackageToProcess, Source, SourceStats, SourceType};
use crate::state::SourceData;
use anyhow::Result;
use anyhow::{anyhow, Context};
use chrono::prelude::*;
use chrono::Duration;
use chrono_humanize::HumanTime;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct RubyGemsSource {
    last_package_timestamp: DateTime<Utc>,
    #[serde(default)]
    stats: SourceStats,
}

#[derive(Deserialize)]
pub struct RubyGemsResponse {
    name: String,
    version: String,
    gem_uri: Url,
    version_created_at: DateTime<Utc>,
}

impl Display for RubyGemsSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RubyGems - Gems updated after {} ({})",
            self.last_package_timestamp,
            HumanTime::from(self.last_package_timestamp - Utc::now())
        )
    }
}

impl Source for RubyGemsSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                last_package_timestamp: "2019-01-18T21:24:29Z".parse().unwrap(),
                stats: Default::default(),
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(&mut self, limit: usize) -> Result<Vec<PackageToProcess>> {
        // https://rubygems.org/api/v1/timeframe_versions.json?from=2019-01-18T21:24:29Z&to=2019-01-18T21:24:31Z
        // https://rubygems.org/api/v1/timeframe_versions.json?from=2019-01-18T21:24:29&to=2019-01-20T21:24:29&page=0
        let end_date = self.last_package_timestamp + Duration::days(5);
        let mut results = vec![];
        let base_url = format!(
            "https://rubygems.org/api/v1/timeframe_versions.json?from={}&to={}",
            self.last_package_timestamp
                .to_rfc3339_opts(SecondsFormat::Secs, true,),
            end_date.to_rfc3339_opts(SecondsFormat::Secs, true,)
        );
        for page in 0..50 {
            if results.len() >= limit {
                break;
            }
            let url = format!("{base_url}&page={page}");
            let response = reqwest::blocking::get(&url)
                .with_context(|| format!("Failed to request {url}"))?;
            let ruby_response: Vec<RubyGemsResponse> = response
                .json()
                .with_context(|| format!("Failed to parse JSON from {url}"))?;
            if ruby_response.is_empty() {
                break;
            } else {
                results.extend(ruby_response)
            }
        }

        let results: Vec<_> = results.into_iter().take(limit).collect();

        let last_date = results
            .iter()
            .map(|v| v.version_created_at)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No gem releases found"))?;

        self.last_package_timestamp = last_date;

        let to_process = results
            .into_iter()
            .map(|v| PackageToProcess {
                download_url: v.gem_uri,
                name: v.name,
                version: v.version,
                source: SourceType::RubyGems,
            })
            .collect();

        Ok(to_process)
    }

    fn to_state(&self) -> Result<SourceData> {
        Ok(serde_json::to_value(self)?)
    }

    fn get_stats(&mut self) -> &mut SourceStats {
        &mut self.stats
    }
}
