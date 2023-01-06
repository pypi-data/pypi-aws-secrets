use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Command;

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

pub fn run_ripgrep(args: &[&str]) -> Result<Vec<RipGrepMatch>> {
    let output = Command::new("rg")
        .args(args)
        .output()
        .with_context(|| format!("Error running rg with args {args:?}"))?;

    if !output.stderr.is_empty() {
        for line in output.stderr.lines().flatten() {
            eprintln!("{line}");
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
    Ok(matches)
}
