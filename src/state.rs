use crate::sources::SourceType;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

pub type SourceData = Value;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct State {
    sources: HashMap<SourceType, SourceData>,
}

impl State {
    pub fn load(path: &PathBuf) -> Result<Self> {
        let state_file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return Ok(State::default()),
        };
        let writer = BufReader::new(state_file);
        Ok(serde_json::from_reader(writer)?)
    }

    pub fn save(self, path: &PathBuf) -> Result<()> {
        let state_file = File::create(path)?;
        let writer = BufWriter::new(state_file);
        Ok(serde_json::to_writer_pretty(writer, &self)?)
    }

    pub fn update_state(&mut self, source: SourceType, value: SourceData) {
        self.sources.insert(source, value);
    }

    pub fn data_for_source(&self, source: &SourceType) -> SourceData {
        match self.sources.get(source) {
            None => SourceData::Null,
            Some(v) => v.clone(),
        }
    }
}
