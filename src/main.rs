use chrono::prelude::*;
use chrono::Duration;
use itertools::Itertools;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};

use clap::Parser;
use rayon::prelude::*;

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use temp_dir::TempDir;
use url::Url;
use xmlrpc::{Request, Value as XmlValue};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectFile {
    pub url: Url,
    pub filename: String,
    #[serde(rename = "upload_time_iso_8601")]
    pub upload_time: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct PackageVersion {
    pub urls: Vec<ProjectFile>,
}

#[derive(Debug)]
pub struct PackageToProcess {
    pub pypi_file: ProjectFile,
    pub temp_dir: TempDir,
    pub download_location: PathBuf,
    pub extract_location: PathBuf,
    pub name: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct FoundKey {
    pub public_path: String,
    pub pypi_file: ProjectFile,
    pub name: String,
    pub version: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RgMatchLines {
    text: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RgMatchPath {
    text: PathBuf,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "lowercase")]
pub enum RgOutput {
    Match {
        line_number: usize,
        lines: RgMatchLines,
        path: RgMatchPath,
    },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct State {
    last_timestamp: DateTime<Utc>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Process {
        #[arg()]
        since: DateTime<Utc>,
    },
    ProcessRelative {
        #[arg()]
        hours: i64,
    },
    ProcessSpecific {
        #[arg()]
        project: String,
        #[arg()]
        version: String,
        #[arg()]
        file_name: String,
    },
}

fn main() {
    let args: Args = Args::parse();
    match args.action {
        Action::ProcessRelative { hours } => {
            let ts: DateTime<Utc> = Utc::now() - Duration::hours(hours);
            let new_files = find_new_pypi_releases(ts);
            let release_info = fetch_release_info(new_files);
            let to_process = download_releases(release_info);
            process(to_process);
        }
        Action::ProcessSpecific {
            project,
            version,
            file_name,
        } => {
            let mut releases = HashMap::new();
            releases.insert((project, version), vec![file_name]);
            let release_info = fetch_release_info(releases);
            let to_process = download_releases(release_info);
            process(to_process)
        }
        Action::Process { since } => {
            let new_files = find_new_pypi_releases(since);
            let release_info = fetch_release_info(new_files);
            let to_process = download_releases(release_info);
            process(to_process);
        }
    };
}

fn update_state(last_timestamp: DateTime<Utc>) {
    let state_file = File::create("state.json").unwrap();
    let writer = BufWriter::new(state_file);
    serde_json::to_writer(writer, &State { last_timestamp }).unwrap();
}

fn read_state() -> State {
    let state_file = File::open("state.json").unwrap();
    let writer = BufReader::new(state_file);
    serde_json::from_reader(writer).unwrap()
}

fn process(items: Vec<PackageToProcess>) {
    let to_continue_processing = find_interesting_packages(items);
    println!(
        "Total interesting packages: {}",
        to_continue_processing.len()
    );
    if to_continue_processing.is_empty() {
        return;
    }
    extract_and_check_keys(to_continue_processing);
}

fn find_interesting_packages(items: Vec<PackageToProcess>) -> Vec<PackageToProcess> {
    unimplemented!()
    // items
    //     .into_par_iter()
    //     .map(|v| {
    //         let output = Command::new("rg")
    //             .args([
    //                 "--pre",
    //                 "./extract.sh",
    //                 "((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))",
    //                 "--threads",
    //                 "1",
    //                 "-m",
    //                 "1",
    //                 "--json",
    //                 v.local_file.to_str().unwrap(),
    //             ])
    //             .output()
    //             .expect("Failed to run rg");
    //         if !output.stderr.is_empty() {
    //             eprintln!("Error! {}", String::from_utf8(output.stderr).unwrap());
    //         } else {
    //             let matches: Vec<RgOutput> = output
    //                 .stdout
    //                 .lines()
    //                 .flatten()
    //                 .flat_map(|line| serde_json::from_str(&line))
    //                 .collect();
    //             if !matches.is_empty() {
    //                 println!("Found {} matches for {:?}", matches.len(), v);
    //                 return Some(v);
    //             }
    //         }
    //         None
    //     })
    //     .flatten()
    //     .collect()
}

fn extract_and_check_keys(items: Vec<PackageToProcess>) {
    unimplemented!()
    // let aws_keys: Vec<_> = items.into_par_iter().map(|p| {
    //     // https://inspector.pypi.io/project/hadata/2.5.111/packages/0e/ec/baf1a440e204e00ddb9fdc9a45cfb7bd0100ac22ae51670e9e4854a1adf2/hadata-2.5.111-py2.py3-none-any.whl/
    //     // https://inspector.pypi.io/project/hadata/2.5.111/packages/0e/ec/baf1a440e204e00ddb9fdc9a45cfb7bd0100ac22ae51670e9e4854a1adf2/hadata-2.5.111-py2.py3-none-any.whl/hautils/hamail.py#line.54
    //     // let output_dir = TempDir::new().unwrap();
    //     let temp_path = p.local_file.path();
    //     // let download_path = temp_path
    //     let _output = Command::new("unar")
    //         .args([
    //             "-k",
    //             "skip",
    //             "-q",
    //             "-o",
    //             output_path.to_str().unwrap(),
    //             p.local_file.path().to_str().unwrap(),
    //         ])
    //         .output()
    //         .expect("Failed to run unar");
    //
    //     let rg_output = Command::new("rg")
    //         .args([
    //             "--multiline",
    //             "-o",
    //             "(('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\").*?(\n^.*?){0,4}(('|\")[a-zA-Z0-9+/]{40}('|\"))|('|\")[a-zA-Z0-9+/]{40}('|\").*?(\n^.*?){0,3}('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\"))",
    //             "--json",
    //             output_path.to_str().unwrap()
    //         ])
    //         .output()
    //         .expect("Failed to run rg");
    //     let matches: Vec<RgOutput> = rg_output
    //         .stdout
    //         .lines()
    //         .flatten()
    //         .flat_map(|line| serde_json::from_str(&line))
    //         .collect();
    //     let mut found = vec![];
    //
    //     // println!("{}", String::from_utf8(rg_output.stdout).unwrap());
    //
    //     for m in matches {
    //         // extract path from extracted path
    //         match m {
    //             RgOutput::Match { line_number, lines, path } => {
    //                 lazy_static! {
    //                         static ref ACCESS_KEY_REGEX: Regex = Regex::new("(('|\")(?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16})('|\"))").unwrap();
    //                         static ref SECRET_KEY_REGEX: Regex = Regex::new("(('|\")([a-zA-Z0-9+/]{40})('|\"))").unwrap();
    //                 }
    //
    //                 let extracted_key_id = ACCESS_KEY_REGEX.find(&lines.text);
    //                 let extracted_secret_key = SECRET_KEY_REGEX.find(&lines.text);
    //
    //                 let (key_match, secret_match) = match (extracted_key_id, extracted_secret_key) {
    //                     (Some(km), Some(sm)) => {
    //                         let key_str = &lines.text[km.range()];
    //                         let secret_str = &lines.text[sm.range()];
    //                         (key_str[1..(key_str.len() - 1)].to_string(), secret_str[1..(secret_str.len() - 1)].to_string())
    //                     }
    //                     _ => {
    //                         eprintln!("Cannot find sub matches for {:?}", p);
    //                         continue;
    //                     }
    //                 };
    //
    //                 println!("Lines: {:?}", lines);
    //                 let temp_component_count = output_path.components().count();
    //                 let path_components: Vec<_> = path.text.components().collect();
    //                 let final_path: &PathBuf = &path_components[temp_component_count..].iter().collect();
    //                 // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic-s3-test-1.0/mathlogic/credentials.py
    //                 // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic/credentials.py
    //                 let public_path = format!("https://inspector.pypi.io/project/{}/{}/{}/{}#line.{}", p.name, p.version, p.pypi_file.url.path(), final_path.to_str().unwrap(), line_number);
    //                 // println!("path: {}", inspector_path);
    //                 found.push(FoundKey {
    //                     public_path,
    //                     pypi_file: p.pypi_file.clone(),
    //                     name: p.name.clone(),
    //                     version: p.version.clone(),
    //                     access_key: key_match,
    //                     secret_key: secret_match,
    //                 })
    //             }
    //         }
    //     }
    //     found
    // }).flatten().collect();
    //
    // // Aws SDK is all async. Bit annoying.
    // let runtime = tokio::runtime::Runtime::new().unwrap();
    // let checker = runtime.spawn(async {
    //     let mut valid_keys = vec![];
    //     println!("Trying keys...");
    //     for key in aws_keys {
    //         println!("Key {:?}", key);
    //         std::env::set_var("AWS_ACCESS_KEY_ID", &key.access_key);
    //         std::env::set_var("AWS_SECRET_ACCESS_KEY", &key.secret_key);
    //         std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");
    //         let config = aws_config::load_from_env().await;
    //         let client = aws_sdk_sts::Client::new(&config);
    //         match client.get_caller_identity().send().await {
    //             Ok(_) => {
    //                 valid_keys.push(key);
    //             }
    //             Err(e) => {
    //                 eprintln!("sts error: {:?}", e);
    //                 continue;
    //             }
    //         }
    //     }
    //     // tokio::time::sleep(core::time::Duration::from_secs(2)).await;
    //     valid_keys
    // });
    // let res = runtime.block_on(checker).unwrap();
    //
    // for valid_key in res {
    //     println!("\nFound key! {:?}", valid_key);
    // }
}

fn find_new_pypi_releases(since: DateTime<Utc>) -> HashMap<(String, String), Vec<String>> {
    let changelog_request = Request::new("changelog").arg(since.timestamp());
    let res = changelog_request
        .call_url("https://pypi.org/pypi")
        .expect("Error");

    let values = if let XmlValue::Array(v) = res {
        v
    } else {
        panic!("Unknown response!")
    };

    values
        .iter()
        .filter_map(|value| match value {
            XmlValue::Array(v) => Some(v),
            _ => None,
        })
        .filter_map(|value| match &value[..] {
            [XmlValue::String(name), XmlValue::String(version), _, XmlValue::String(action)]
                if action.starts_with("add ") && !action.ends_with(".exe") =>
            {
                let file_name = action.split(' ').last().unwrap();
                Some((name.clone(), version.clone(), file_name.to_string()))
            }
            _ => None,
        })
        .sorted()
        .group_by(|(name, version, _)| (name.clone(), version.clone()))
        .into_iter()
        .map(|(key, value)| (key, value.map(|(_, _, f)| f).collect()))
        .collect()
}

fn fetch_release_info(
    grouped_uploads: HashMap<(String, String), Vec<String>>,
) -> Vec<(String, String, ProjectFile)> {
    grouped_uploads
        .into_par_iter()
        .filter_map(|((name, version), files)| {
            let url = format!("https://pypi.org/pypi/{name}/{version}/json");
            let response = match reqwest::blocking::get(&url) {
                Ok(v) => v,
                Err(_e) => {
                    println!("error!");
                    return None;
                }
            };

            if response.status() == 404 {
                return None;
            }

            let mut original_json_response: serde_json::Value =
                response.json().expect("Error getting JSON");
            let info = original_json_response
                .get_mut("info")
                .unwrap()
                .as_object_mut()
                .unwrap();
            info.remove("description");
            let package_info: PackageVersion =
                serde_json::from_value(original_json_response.clone())
                    .unwrap_or_else(|_| panic!("Error: {}", url));

            let matches: Vec<_> = package_info
                .urls
                .into_iter()
                .filter(|v| files.contains(&v.filename))
                .map(|v| (name.clone(), version.clone(), v))
                .collect();
            Some(matches)
        })
        .flatten()
        .collect()
}

fn download_releases(releases: Vec<(String, String, ProjectFile)>) -> Vec<PackageToProcess> {
    releases
        .into_par_iter()
        .map(|(name, version, file)| {
            let temp_dir = TempDir::new().unwrap();
            let download_location = temp_dir.path().join("download").join(&file.filename);
            let extract_location = temp_dir.path().join("extracted");
            let mut out = File::create(&download_location).unwrap();
            let mut resp = reqwest::blocking::get(file.url.clone()).unwrap();
            io::copy(&mut resp, &mut out).expect("Error copying");
            PackageToProcess {
                pypi_file: file.clone(),
                temp_dir: TempDir::new().unwrap(),
                download_location,
                extract_location,
                name: name.clone(),
                version: version.clone(),
            }
        })
        .collect()
}
