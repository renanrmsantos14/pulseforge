use crate::config::model::{Profile, SinkInfo};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio backend unavailable: {0}")]
    Unavailable(String),
    #[error("output not found: {0}")]
    OutputNotFound(String),
}

pub trait AudioBackend {
    fn list_outputs(&self) -> Result<Vec<SinkInfo>, AudioError>;
    fn default_output(&self) -> Result<Option<SinkInfo>, AudioError>;
    fn attach_filter(&mut self, output: &SinkInfo) -> Result<(), AudioError>;
    fn detach_filter(&mut self) -> Result<(), AudioError>;
    fn apply_profile(&mut self, profile: &Profile) -> Result<(), AudioError>;
}
