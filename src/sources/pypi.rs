use crate::sources::{PackageToProcess, Source, SourceStats, SourceType};
use crate::state::SourceData;
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, TimeZone, Utc};
use chrono_humanize::HumanTime;
use itertools::Itertools;
use rand::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use url::Url;
use xmlrpc::{Request, Value as XmlValue, Value};

#[derive(Serialize, Deserialize)]
pub struct PyPiSource {
    changelog_serial: u64,
    last_package_timestamp: Option<DateTime<Utc>>,
    #[serde(default)]
    stats: SourceStats,
}

impl Display for PyPiSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PyPi - Packages changed since serial {}",
            self.changelog_serial
        )?;
        if let Some(ts) = self.last_package_timestamp {
            write!(
                f,
                ". Last timestamp: {} ({})",
                ts,
                HumanTime::from(ts - Utc::now())
            )?;
        }
        Ok(())
    }
}

impl Source for PyPiSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                changelog_serial: 0,
                last_package_timestamp: None,
                stats: Default::default(),
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(&mut self, limit: usize) -> Result<Vec<PackageToProcess>> {
        let changelog_request =
            Request::new("changelog_since_serial").arg(self.changelog_serial as i32);
        let res = changelog_request.call_url("https://pypi.org/pypi")?;
        let changelog_items: Vec<_> = match res {
            Value::Array(items) => {
                let only_xml_vecs = items.iter().filter_map(|item| match item {
                    XmlValue::Array(v) => Some(v),
                    _ => None,
                });
                only_xml_vecs
                    .filter_map(parse_changelog_item)
                    .take(limit)
                    .collect()
            }
            _ => {
                bail!("Unknown changelog response: {:?}", res);
            }
        };
        let highest_serial = changelog_items
            .iter()
            .map(|v| v.serial)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No changelog items found"))?;
        let highest_datetime = changelog_items
            .iter()
            .map(|v| v.ts)
            .max_by_key(|v| *v)
            .ok_or_else(|| anyhow!("No changelog items found"))?;

        println!("Highest timestamp: {}", highest_datetime);

        self.changelog_serial = highest_serial;
        self.last_package_timestamp = Some(highest_datetime);

        // Now we have a vec of individual releases. We need to fetch the download URLs, which requires
        // us to make 1 request per _package version_ to fetch N _releases_.
        // To do this we create a hashmap mapping (name, version) -> [files].
        let changelogs_by_packages = changelog_items
            .into_iter()
            .map(|v| ((v.package_name.clone(), v.version.clone()), v))
            .into_group_map();

        println!(
            "Fetching pypi package info for {} packages",
            changelogs_by_packages.len()
        );
        let packages_to_process: Result<Vec<_>> = changelogs_by_packages
            .into_par_iter()
            .map(|((name, version), changelogs)| {
                println!("{} - {}", name, changelogs[0].serial);
                fetch_download_url_for_package(name, version, changelogs)
            })
            .collect();
        let mut flattened_packages: Vec<_> = packages_to_process?.into_iter().flatten().collect();
        flattened_packages.shuffle(&mut thread_rng());
        Ok(flattened_packages)
    }

    fn to_state(&self) -> Result<SourceData> {
        Ok(serde_json::to_value(self)?)
    }

    fn get_stats(&mut self) -> &mut SourceStats {
        &mut self.stats
    }
}

struct ChangelogItem {
    package_name: String,
    version: String,
    file_name: String,
    ts: DateTime<Utc>,
    serial: u64,
}

fn parse_changelog_item(value: &Vec<XmlValue>) -> Option<ChangelogItem> {
    match &value[..] {
        [XmlValue::String(name), XmlValue::String(version), XmlValue::Int(ts), XmlValue::String(action), XmlValue::Int(serial)]
            if action.starts_with("add ") && !action.ends_with(".exe") =>
        {
            let file_name = action.split(' ').last().unwrap();
            Some(ChangelogItem {
                package_name: name.clone(),
                version: version.clone(),
                ts: Utc.timestamp_opt(*ts as i64, 0).unwrap(),
                file_name: file_name.to_string(),
                serial: (*serial) as u64,
            })
        }
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct PyPiResponse {
    urls: Vec<PackageUrl>,
}

#[derive(Deserialize)]
pub struct PackageUrl {
    url: String,
    filename: String,
}

fn fetch_download_url_for_package(
    name: String,
    version: String,
    changelogs: Vec<ChangelogItem>,
) -> Result<Vec<PackageToProcess>> {
    let url = format!("https://pypi.org/pypi/{name}/{version}/json");
    let response = reqwest::blocking::get(url)?;
    if response.status() == 404 {
        return Ok(vec![]);
    }

    let file_names: HashSet<_> = changelogs.into_iter().map(|c| c.file_name).collect();

    let response: PyPiResponse = response.json()?;

    let matching_urls = response
        .urls
        .into_iter()
        .filter(|v| file_names.contains(&v.filename))
        .filter_map(|v| Url::parse(&v.url).ok());

    Ok(matching_urls
        .map(|url| PackageToProcess::new(name.clone(), version.clone(), url, SourceType::PyPi))
        .collect())
}
