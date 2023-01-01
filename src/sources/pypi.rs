use crate::sources::{PackageToProcess, Source, SourceType};
use crate::state::SourceData;
use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use rand::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;
use xmlrpc::{Request, Value as XmlValue, Value};

#[derive(Serialize, Deserialize)]
pub struct PyPiSource {
    changelog_serial: u64,
}

impl Source for PyPiSource {
    fn new(data: SourceData) -> Result<Self> {
        match data {
            SourceData::Null => Ok(Self {
                changelog_serial: 0,
            }),
            _ => Ok(serde_json::from_value(data)?),
        }
    }

    fn get_new_packages_to_process(
        &self,
        limit: usize,
    ) -> Result<(SourceData, Vec<PackageToProcess>)> {
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
        let new_state = serde_json::to_value(PyPiSource {
            changelog_serial: highest_serial,
        })?;

        // Now we have a vec of individual releases. We need to fetch the download URLs, which requires
        // us to make 1 request per _package version_ to fetch N _releases_.
        // To do this we create a hashmap mapping (name, version) -> [files].
        let changelogs_by_packages = changelog_items
            .into_iter()
            .map(|v| ((v.package_name.clone(), v.version.clone()), v))
            .into_group_map();

        let packages_to_process: Result<Vec<_>> = changelogs_by_packages
            .into_par_iter()
            .map(|((name, version), changelogs)| {
                fetch_download_url_for_package(name, version, changelogs)
            })
            .collect();
        let mut flattened_packages: Vec<_> = packages_to_process?.into_iter().flatten().collect();
        flattened_packages.shuffle(&mut thread_rng());
        Ok((new_state, flattened_packages))
    }
}

struct ChangelogItem {
    package_name: String,
    version: String,
    file_name: String,
    serial: u64,
}

fn parse_changelog_item(value: &Vec<XmlValue>) -> Option<ChangelogItem> {
    match &value[..] {
        [XmlValue::String(name), XmlValue::String(version), _, XmlValue::String(action), XmlValue::Int(serial)]
            if action.starts_with("add ") && !action.ends_with(".exe") =>
        {
            let file_name = action.split(' ').last().unwrap();
            Some(ChangelogItem {
                package_name: name.clone(),
                version: version.clone(),
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
