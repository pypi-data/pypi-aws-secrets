use crate::sources::SourceType;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

pub type SourceData = Map<String, Value>;

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct State {
    sources: HashMap<SourceType, SourceData>,
}

impl State {
    pub fn load() -> Result<Self> {
        let state_file = match File::open("state.json") {
            Ok(f) => f,
            Err(_) => return Ok(State::default())
        };
        let writer = BufReader::new(state_file);
        Ok(serde_json::from_reader(writer)?)
    }

    pub fn save(self) -> Result<()> {
        let state_file = File::create("state.json")?;
        let writer = BufWriter::new(state_file);
        Ok(serde_json::to_writer(writer, &self)?)
    }

    pub fn update_state(&mut self, source: SourceType, value: SourceData) {
        self.sources.insert(source, value);
    }

    pub fn data_for_source(&self, source: SourceType) -> SourceData {
        match self.sources.get(&source) {
            None => SourceData::default(),
            Some(v) => v.clone()
        }
    }
}
