use chrono::prelude::*;
use chrono::Duration;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fs::{DirEntry, File};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, io};
use std::io::{BufReader, BufWriter};
use aws_sdk_sts::error::GetCallerIdentityError;
use aws_sdk_sts::output::GetCallerIdentityOutput;
use aws_sdk_sts::types::SdkError;

use clap::Parser;
use rayon::prelude::*;
use reqwest::StatusCode;
use serde_json;
use serde_json::Value as JsonValue;
use xmlrpc::{Request, Value as XmlValue};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use temp_dir::TempDir;
use url::Url;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProjectFile {
    pub url: Url,
    pub filename: String,
    #[serde(rename="upload_time_iso_8601")]
    pub upload_time: DateTime<Utc>
}

#[derive(Deserialize, Debug)]
pub struct PackageVersion {
    pub urls: Vec<ProjectFile>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PackageToProcess {
    pub pypi_file: ProjectFile,
    pub local_file: PathBuf,
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
    Download {
        #[arg()]
        downloads_dir: PathBuf,
        #[arg()]
        since: DateTime<Utc>,
    },
    DownloadIncrement {
        #[arg()]
        downloads_dir: PathBuf,
    },
    DownloadRelative {
        #[arg()]
        downloads_dir: PathBuf,
        #[arg()]
        hours: i64,
    },
    DownloadSpecific {
        #[arg()]
        downloads_dir: PathBuf,
        #[arg()]
        project: String,
        #[arg()]
        version: String,
        #[arg()]
        file_name: String,
    },
    Process {
        #[arg()]
        state: PathBuf,
    },
}

fn main() {
    let args: Args = Args::parse();
    match args.action {
        Action::Download {
            downloads_dir,
            since,
        } => {
            download(downloads_dir, since);
        },
        Action::DownloadIncrement { downloads_dir } => {
            let state = read_state();
            let new_time = download(downloads_dir, state.last_timestamp);
            update_state(new_time);
        }
        Action::DownloadRelative {
            downloads_dir,
            hours,
        } => {
            let ts: DateTime<Utc> = Utc::now() - Duration::hours(hours);
            download(downloads_dir, ts);
        }
        Action::DownloadSpecific {
            downloads_dir,
            project,
            version,
            file_name,
        } => {
            let file_name_str = file_name.as_str();
            let mut releases = HashMap::new();
            releases.insert((&project, &version), vec![file_name_str]);
            download_releases(releases, downloads_dir);
        }
        Action::Process { state } => {
            process(state);
        }
    };
}

fn update_state(last_timestamp: DateTime<Utc>) {
    let state_file = File::create("state.json").unwrap();
    let writer = BufWriter::new(state_file);
    serde_json::to_writer(writer, &State{ last_timestamp }).unwrap();
}

fn read_state() -> State {
    let state_file = File::open("state.json").unwrap();
    let writer = BufReader::new(state_file);
    serde_json::from_reader(writer).unwrap()
}

fn process(state: PathBuf) {
    let state_file = fs::File::open(state).expect("Error reading state");
    let reader = io::BufReader::new(state_file);
    let items: Vec<_> = serde_json::Deserializer::from_reader(reader)
        .into_iter::<PackageToProcess>()
        .flatten()
        .collect();
    let to_continue_processing: Vec<_> = items
        .into_par_iter()
        .map(|v| {
            let output = Command::new("rg")
                .args([
                    "--pre",
                    "./extract.sh",
                    "((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))",
                    "--threads",
                    "1",
                    "-m",
                    "1",
                    "--json",
                    v.local_file.to_str().unwrap(),
                ])
                .output()
                .expect("Failed to run rg");
            if !output.stderr.is_empty() {
                eprintln!("Error! {}", String::from_utf8(output.stderr).unwrap());
            } else {
                // let out_json_lines: Vec<_> = .collect();
                let matches: Vec<RgOutput> = output
                    .stdout
                    .lines()
                    .flatten()
                    .map(|line| serde_json::from_str(&line))
                    .flatten()
                    .collect();
                if !matches.is_empty() {
                    println!("Found matches for {:?}", v);
                    for m in matches {
                        println!(" - {:?}", m);
                    }
                    return Some(v);
                }
            }
            None
        })
        .flatten()
        .collect();
    println!(
        "Total interesting packages: {}",
        to_continue_processing.len()
    );
    if to_continue_processing.is_empty() {
        return;
    }

    let aws_keys: Vec<_> = to_continue_processing.into_par_iter().map(|p| {
        let output_dir = TempDir::new().unwrap();
        let output_path = output_dir.path();
        let output = Command::new("unar")
            .args([
                "-k",
                "skip",
                "-q",
                "-o",
                output_path.to_str().unwrap(),
                p.local_file.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to run unar");

        let rg_output = Command::new("rg")
            .args([
                "--multiline",
                "-o",
                "(('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\").*?(\n^.*?){0,4}(('|\")[a-zA-Z0-9+/]{40}('|\"))|('|\")[a-zA-Z0-9+/]{40}('|\").*?(\n^.*?){0,3}('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\"))",
                "--json",
                output_path.to_str().unwrap()
            ])
            .output()
            .expect("Failed to run rg");
        let matches: Vec<RgOutput> = rg_output
            .stdout
            .lines()
            .flatten()
            .map(|line| serde_json::from_str(&line))
            .flatten()
            .collect();
        let mut found = vec![];

        // println!("{}", String::from_utf8(rg_output.stdout).unwrap());

        for m in matches {
            // extract path from extracted path
            match m {
                RgOutput::Match { line_number, lines, path } => {
                    lazy_static! {
                            static ref ACCESS_KEY_REGEX: Regex = Regex::new("(('|\")(?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16})('|\"))").unwrap();
                            static ref SECRET_KEY_REGEX: Regex = Regex::new("(('|\")([a-zA-Z0-9+/]{40})('|\"))").unwrap();
                    }

                    let extracted_key_id = ACCESS_KEY_REGEX.find(&lines.text);
                    let extracted_secret_key = SECRET_KEY_REGEX.find(&lines.text);

                    let (key_match, secret_match) = match (extracted_key_id, extracted_secret_key) {
                        (Some(km), Some(sm)) => {
                            let key_str = &lines.text[km.range()];
                            let secret_str = &lines.text[sm.range()];
                            (key_str[1..(key_str.len() - 1)].to_string(), secret_str[1..(secret_str.len() - 1)].to_string())
                        }
                        _ => {
                            eprintln!("Cannot find sub matches for {:?}", p);
                            continue;
                        }
                    };

                    println!("Lines: {:?}", lines);
                    let temp_component_count = output_path.components().count();
                    let path_components: Vec<_> = path.text.components().collect();
                    let final_path: &PathBuf = &path_components[temp_component_count..].iter().collect();
                    // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic-s3-test-1.0/mathlogic/credentials.py
                    // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic/credentials.py
                    let public_path = format!("https://inspector.pypi.io/project/{}/{}/{}/{}#line.{}", p.name, p.version, p.pypi_file.url.path(), final_path.to_str().unwrap(), line_number);
                    // println!("path: {}", inspector_path);
                    found.push(FoundKey {
                        public_path,
                        pypi_file: p.pypi_file.clone(),
                        name: p.name.clone(),
                        version: p.version.clone(),
                        access_key: key_match,
                        secret_key: secret_match,
                    })
                }
            }
        }
        found
    }).flatten().collect();

    // Aws SDK is all async. Bit annoying.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let checker = runtime.spawn(async {
        let mut  valid_keys = vec![];
        println!("Trying keys...");
        for key in aws_keys {
            std::env::set_var("AWS_ACCESS_KEY_ID", &key.access_key);
            std::env::set_var("AWS_SECRET_ACCESS_KEY", &key.secret_key);
            let config = aws_config::load_from_env().await;
            let client = aws_sdk_sts::Client::new(&config);
            match  client.get_caller_identity().send().await {
                Ok(_) => {
                    valid_keys.push(key);
                }
                Err(e) => {
                    eprintln!("sts error: {}", e);
                    continue
                }
            }

        }
        // tokio::time::sleep(core::time::Duration::from_secs(2)).await;
        return valid_keys
    });
    let res = runtime.block_on(checker).unwrap();

    for valid_key in res {
        println!("\nFound key! {:?}", valid_key);
    }
}


fn download(downloads_dir: PathBuf, since: DateTime<Utc>) -> DateTime<Utc> {
    // fs::create_dir(&args.downloads_dir).unwrap();

    // let ts: DateTime<Utc> = Utc::now() - Duration::hours(1);
    let changelog_request = Request::new("changelog").arg(since.timestamp());
    let res = changelog_request
        .call_url("https://pypi.org/pypi")
        .expect("Error");

    let values = if let XmlValue::Array(v) = res {
        v
    } else {
        panic!("Unknown response!")
    };

    let new_uploads: Vec<(&String, &String, &str)> = values
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
                    Some((name, version, file_name))
                }
            _ => None,
        })
        .sorted()
        .collect();

    let grouped_uploads: HashMap<_, Vec<_>> = new_uploads
        .into_iter()
        .group_by(|(name, version, _)| (*name, *version))
        .into_iter()
        .map(|(key, value)| (key, value.map(|(_, _, f)| f).collect()))
        .collect();

    eprintln!("Changed releases: {:?}", grouped_uploads.len());
    download_releases(grouped_uploads, downloads_dir)
}

fn download_releases(releases: HashMap<(&String, &String), Vec<&str>>, downloads_dir: PathBuf) -> DateTime<Utc> {
    let mut download_times: Vec<_> = releases
        .into_par_iter()
        .filter_map(|((name, version), files)| {
            let url = format!("https://pypi.org/pypi/{name}/{version}/json");
            let response = match reqwest::blocking::get(&url) {
                Ok(v) => v,
                Err(e) => {
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
                    .expect(format!("Error: {}", url).as_str());

            let mut matches: Vec<_> = package_info
                .urls
                .into_iter()
                .filter(|v| files.contains(&&&v.filename.as_str()))
                .map(|v| (name, version, v))
                .collect();
            Some(matches)
            // Some((original_json_response, matches))
        })
        .flatten()
        .map(|(name, version, file)| {
            let output_location = downloads_dir.join(&file.filename);
            let mut out = File::create(&output_location).unwrap();
            let mut resp = reqwest::blocking::get(file.url.clone()).unwrap();
            io::copy(&mut resp, &mut out).expect("Error copying");
            println!(
                "{}",
                serde_json::to_string(&PackageToProcess {
                    pypi_file: file.clone(),
                    local_file: output_location,
                    name: name.clone(),
                    version: version.clone(),
                })
                    .unwrap()
            );
            file.upload_time
        }).collect();
    download_times.sort();
    return download_times[download_times.len() -1]
}
