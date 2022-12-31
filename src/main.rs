use chrono::prelude::*;
use chrono::Duration;
use indicatif::ParallelProgressIterator;
use itertools::Itertools;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

use lazy_static::lazy_static;
use std::process::Command;
use std::{fs, io};

use clap::Parser;
use rayon::prelude::*;

use serde::{Deserialize, Serialize};
use temp_dir::TempDir;
use tinytemplate::TinyTemplate;
use url::Url;
use xmlrpc::{Request, Value as XmlValue};

#[derive(Deserialize, Serialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FoundKey {
    pub public_path: String,
    pub pypi_file: ProjectFile,
    pub name: String,
    pub version: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LiveKey {
    pub key: FoundKey,
    pub role_name: String,
}

#[derive(Deserialize, Debug)]
pub struct RgMatchLines {
    text: String,
}

#[derive(Deserialize, Debug)]
pub struct RgMatchPath {
    text: PathBuf,
}

#[derive(Deserialize, Debug)]
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
    Process {},
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
            let (_, new_files) = find_new_pypi_releases(ts);
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
            let matches = process(to_process);
            create_findings(matches);
        }
        Action::Process {} => {
            let state = read_state();
            println!("Processing from {}", state.last_timestamp);
            let (max_ts, new_files) = find_new_pypi_releases(state.last_timestamp);
            let release_info = fetch_release_info(new_files);
            let to_process = download_releases(release_info);
            process(to_process);
            update_state(max_ts);
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

fn create_findings(items: Vec<LiveKey>) {
    let mut template = TinyTemplate::new();
    template
        .add_template("markdown", include_str!("template.md"))
        .unwrap();

    // A single package may contain multiple keys. We ideally want a single file per release file,
    // So we need to sort and group the keys.

    let sorted_keys = items
        .into_iter()
        .sorted_by_key(|v| v.key.pypi_file.filename.clone())
        .group_by(|v| v.key.pypi_file.clone());

    for (project_file, keys) in &sorted_keys {
        #[derive(Serialize)]
        struct TemplateContext {
            project_file: ProjectFile,
            first_key: FoundKey,
            keys: Vec<LiveKey>,
        }

        let keys: Vec<_> = keys
            .into_iter()
            .unique_by(|v| (v.key.access_key.clone(), v.key.secret_key.clone()))
            .collect();
        let first_key = keys.first().unwrap().key.clone();

        let ctx = TemplateContext {
            project_file,
            first_key: first_key.clone(),
            keys,
        };

        let rendered = template.render("markdown", &ctx).unwrap();
        let output_dir = PathBuf::from(format!("keys/{}/", first_key.name));
        let output_path = output_dir.join(format!("{}.md", first_key.pypi_file.filename));
        let _ = fs::create_dir_all(output_dir);
        fs::write(&output_path, rendered).unwrap();
        println!("Created file {:?}", output_path);
    }
}

fn process(items: Vec<PackageToProcess>) -> Vec<LiveKey> {
    let to_continue_processing = find_interesting_packages(items);
    println!(
        "Total interesting packages: {}",
        to_continue_processing.len()
    );
    if to_continue_processing.is_empty() {
        return vec![];
    }
    extract_and_check_keys(to_continue_processing)
}

fn find_interesting_packages(items: Vec<PackageToProcess>) -> Vec<PackageToProcess> {
    items
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
                    v.download_location.to_str().unwrap(),
                ])
                .output()
                .expect("Failed to run rg");
            if !output.stderr.is_empty() {
                eprintln!("Error! {}", String::from_utf8(output.stderr).unwrap());
            } else {
                let matches: Vec<RgOutput> = output
                    .stdout
                    .lines()
                    .flatten()
                    .flat_map(|line| serde_json::from_str(&line))
                    .collect();
                if !matches.is_empty() {
                    println!("Found {} matches for {:?}", matches.len(), v);
                    return Some(v);
                }
            }
            None
        })
        .flatten()
        .collect()
}

fn extract_and_check_keys(items: Vec<PackageToProcess>) -> Vec<LiveKey> {
    let aws_keys: Vec<_> = items.into_par_iter().progress().map(|p| {
        // https://inspector.pypi.io/project/hadata/2.5.111/packages/0e/ec/baf1a440e204e00ddb9fdc9a45cfb7bd0100ac22ae51670e9e4854a1adf2/hadata-2.5.111-py2.py3-none-any.whl/
        // https://inspector.pypi.io/project/hadata/2.5.111/packages/0e/ec/baf1a440e204e00ddb9fdc9a45cfb7bd0100ac22ae51670e9e4854a1adf2/hadata-2.5.111-py2.py3-none-any.whl/hautils/hamail.py#line.54
        let _output = Command::new("unar")
            .args([
                "-D",
                "-k",
                "skip",
                "-q",
                "-o",
                p.extract_location.to_str().unwrap(),
                p.download_location.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to run unar");

        let rg_output = Command::new("rg")
            .args([
                "--multiline",
                "-o",
                "(('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\").*?(\n^.*?){0,4}(('|\")[a-zA-Z0-9+/]{40}('|\"))|('|\")[a-zA-Z0-9+/]{40}('|\").*?(\n^.*?){0,3}('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\"))",
                "--json",
                p.extract_location.to_str().unwrap()
            ])
            .output()
            .expect("Failed to run rg");
        let matches: Vec<RgOutput> = rg_output
            .stdout
            .lines()
            .flatten()
            .flat_map(|line| serde_json::from_str(&line))
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
                    let extract_path_str = p.extract_location.to_str().unwrap();
                    let relative_path = path.text.to_str().unwrap().strip_prefix(&format!("{}/", extract_path_str)).unwrap();
                    // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic-s3-test-1.0/mathlogic/credentials.py
                    // https://inspector.pypi.io/project/mathlogic-s3-test/1.0/packages/0e/0e/4ff410fa20299ced4b88806191b015de257e0bf77617900147a548324774/mathlogic-s3-test-1.0.tar.gz/mathlogic/credentials.py
                    let public_path = format!("https://inspector.pypi.io/project/{}/{}/{}/{}#line.{}", p.name, p.version, p.pypi_file.url.path().strip_prefix('/').unwrap(), relative_path, line_number);
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
        let mut valid_keys = vec![];
        println!("Trying keys...");
        for key in aws_keys {
            println!("Key {:?}", key);
            std::env::set_var("AWS_ACCESS_KEY_ID", &key.access_key);
            std::env::set_var("AWS_SECRET_ACCESS_KEY", &key.secret_key);
            std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");
            let config = aws_config::load_from_env().await;
            let client = aws_sdk_sts::Client::new(&config);
            match client.get_caller_identity().send().await {
                Ok(identity) => {
                    let arn = identity.arn().unwrap();
                    let identity_without_account = arn.split(':').last().unwrap();
                    valid_keys.push(LiveKey {
                        key,
                        role_name: identity_without_account.to_string(),
                    });
                }
                Err(e) => {
                    eprintln!("sts error: {:?}", e);
                    continue;
                }
            }
        }
        valid_keys
    });
    let res = runtime.block_on(checker).unwrap();

    println!("Found {} keys", res.len());
    res
}

fn find_new_pypi_releases(
    since: DateTime<Utc>,
) -> (DateTime<Utc>, HashMap<(String, String), Vec<String>>) {
    let changelog_request = Request::new("changelog").arg(since.timestamp());
    let res = changelog_request
        .call_url("https://pypi.org/pypi")
        .expect("Error");

    let values = if let XmlValue::Array(v) = res {
        v
    } else {
        panic!("Unknown response!")
    };

    let items: Vec<_> = values
        .iter()
        .filter_map(|value| match value {
            XmlValue::Array(v) => Some(v),
            _ => None,
        })
        .filter_map(|value| match &value[..] {
            [XmlValue::String(name), XmlValue::String(version), XmlValue::Int(v), XmlValue::String(action)]
            if action.starts_with("add ") && !action.ends_with(".exe") =>
                {
                    let file_name = action.split(' ').last().unwrap();
                    Some((name.clone(), version.clone(), file_name.to_string(), Utc.timestamp_opt((*v).into(), 0).unwrap()))
                }
            _ => None,
        }).collect();

    let last_timestamp = *items.iter().map(|(_, _, _, ts)| ts).max().unwrap();
    let items_without_timestamp = items.into_iter().map(|(n, v, f, _)| (n, v, f));

    let items_hashmap = items_without_timestamp
        .sorted()
        .group_by(|(name, version, _)| (name.clone(), version.clone()))
        .into_iter()
        .map(|(key, value)| (key, value.map(|(_, _, f)| f).collect()))
        .collect();

    (last_timestamp, items_hashmap)
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
        .progress()
        .map(|(name, version, file)| {
            let temp_dir = TempDir::new().unwrap();
            let download_location = temp_dir.path().join("download").join(&file.filename);
            let extract_location = temp_dir.path().join("extracted");
            fs::create_dir(&extract_location).unwrap();
            fs::create_dir(temp_dir.path().join("download")).unwrap();
            let mut out = File::create(&download_location).unwrap();
            let mut resp = reqwest::blocking::get(file.url.clone()).unwrap();
            io::copy(&mut resp, &mut out).expect("Error copying");
            PackageToProcess {
                pypi_file: file,
                temp_dir,
                download_location,
                extract_location,
                name,
                version,
            }
        })
        .collect()
}
