use anyhow::{Context, Result};
use clap::ColorChoice;
use serde::Deserialize;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Command;
use grep::cli::CommandReaderBuilder;
use grep::printer::{JSONBuilder, StandardBuilder};
use grep::regex::RegexMatcher;
use grep::searcher::{BinaryDetection, SearcherBuilder};

// Search for anything that looks like an AWS access key ID
const QUICK_CHECK_REGEX: &str = "((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))";

// This is a bit ridiculous, but it searches for the AWS access key pattern above combined with
// the secret key regex ([a-zA-Z0-9+/]{40}). This regex is too general, so we need to pair it with
// an access key match that is between 0 and 4 lines of the secret key match.
// It only looks for keys surrounded by quotes, else the false positive rate is too large.
// It also supports secret keys being defined before the access key.
const FULL_CHECK_REGEX: &str = "(('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\").*?(\n^.*?){0,4}(('|\")[a-zA-Z0-9+/]{40}('|\"))+|('|\")[a-zA-Z0-9+/]{40}('|\").*?(\n^.*?){0,3}('|\")((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))('|\"))+";

#[derive(Deserialize, Debug)]
pub struct RgMatchLines {
    pub text: String,
}

#[derive(Deserialize, Debug)]
pub struct RgMatchPath {
    pub text: PathBuf,
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

#[derive(Debug, Clone)]
pub struct RipGrepMatch {
    pub lines: String,
    pub line_number: usize,
    pub path: PathBuf,
}

// use grep::cli;
// use grep::printer::{ColorSpecs, StandardBuilder};
// use grep::regex::RegexMatcher;
// use grep::searcher::{BinaryDetection, SearcherBuilder};

pub fn run_quick_search(path: &PathBuf) -> Result<Vec<RipGrepMatch>> {
    let now = std::time::Instant::now();
    let matcher = RegexMatcher::new(FULL_CHECK_REGEX).unwrap();
    let mut searcher = SearcherBuilder::new()
        .multi_line(true)
        .binary_detection(BinaryDetection::quit(b'\x00')).build();

    let mut printer = JSONBuilder::new().build(vec![]);

    let mut cmd = Command::new("./scripts/extract-stdout.sh");
    cmd.arg(path);
    let mut reader = CommandReaderBuilder::new().build(&mut cmd)?;
    let result = searcher.search_reader(&matcher, reader, printer.sink(&matcher))?;
    println!("Internal RG elapsed: {:.2?}", now.elapsed());
    println!("Internal RG output: {}", String::from_utf8(printer.get_mut().to_owned()).unwrap());

    let matches = run_ripgrep(&[
        "--pre",
        "./scripts/extract-stdout.sh",
        QUICK_CHECK_REGEX,
        "--threads",
        "1",
        "-m",
        "1",
        "--json",
        path.to_str().unwrap(),
    ])?;
    Ok(matches)
}

pub fn run_full_check(path: &PathBuf) -> Result<Vec<RipGrepMatch>> {
    let matches = run_ripgrep(&[
        "--multiline",
        "-o",
        FULL_CHECK_REGEX,
        "--json",
        path.to_str().unwrap(),
    ])?;
    Ok(matches)
}

fn run_ripgrep(args: &[&str]) -> Result<Vec<RipGrepMatch>> {
    let now = std::time::Instant::now();
    //
    // let mut searcher = SearcherBuilder::new()
    //     .binary_detection(BinaryDetection::quit(b'\x00'))
    //     .
    //     .line_number(false)
    //     .build();
    // // let mut printer = StandardBuilder::new().build(cli::stdout(ColorChoice::Never));
    // searcher.search_reader()

    let output = Command::new("rg")
        .args(args)
        .output()
        .with_context(|| format!("Error running rg with args {:?}", args))?;

    if !output.stderr.is_empty() {
        for line in output.stderr.lines().flatten() {
            eprintln!("{}", line);
        }
    }

    let matches = output
        .stdout
        .lines()
        .flatten()
        .flat_map(|line| serde_json::from_str::<RgOutput>(&line))
        .map(|output| match output {
            RgOutput::Match {
                line_number,
                lines,
                path,
            } => RipGrepMatch {
                lines: lines.text,
                line_number,
                path: path.text,
            },
        })
        .collect();
    println!("External RG elapsed: {:.2?}", now.elapsed());
    Ok(matches)
}
