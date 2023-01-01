use crate::sources::{PackageToProcess, Source, SourceType};
use crate::state::SourceData;
use anyhow::anyhow;
use anyhow::Result;
use chrono::prelude::*;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct RubyGemsSource {
    last_date: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct RubyGemsResponse {
    name: String,
    version: String,
    gem_uri: Url,
    version_created_at: DateTime<Utc>,
}

impl Source for RubyGemsSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                last_date: "2019-01-18T21:24:29Z".parse().unwrap(),
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(
        &self,
        limit: usize,
    ) -> anyhow::Result<(SourceData, Vec<PackageToProcess>)> {
        // https://rubygems.org/api/v1/timeframe_versions.json?from=2019-01-18T21:24:29Z&to=2019-01-18T21:24:31Z
        // https://rubygems.org/api/v1/timeframe_versions.json?from=2019-01-18T21:24:29&to=2019-01-20T21:24:29&page=0
        let end_date = self.last_date + Duration::days(2);
        let mut results = vec![];
        let base_url = format!(
            "https://rubygems.org/api/v1/timeframe_versions.json?from={}&to={}",
            self.last_date.to_rfc3339_opts(SecondsFormat::Secs, true,),
            end_date.to_rfc3339_opts(SecondsFormat::Secs, true,)
        );
        for page in 0..10 {
            if results.len() >= limit {
                break;
            }
            let url = format!("{}&page={}", base_url, page);
            let response = reqwest::blocking::get(url)?;
            let ruby_response: Vec<RubyGemsResponse> = response.json()?;
            if ruby_response.is_empty() {
                break;
            } else {
                results.extend(ruby_response)
            }
        }

        let last_date = results
            .iter()
            .map(|v| v.version_created_at)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No gem releases found"))?;

        let new_state = serde_json::to_value(RubyGemsSource { last_date })?;

        let to_process = results
            .into_iter()
            .map(|v| PackageToProcess {
                download_url: v.gem_uri,
                name: v.name,
                version: v.version,
                source: SourceType::RubyGems,
            })
            .collect();

        Ok((new_state, to_process))
    }
}
