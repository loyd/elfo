use derive_more::{Display, Error};
use serde_json::Error as JsonError;
use smartstring::alias::String;

#[derive(Debug, Display, Error)]
pub(crate) struct InspectorError {
    message: String,
}

#[derive(Debug, Display, Error)]
pub(crate) enum UpdateError {
    /// HTTP 429.
    TooManyRequests,
    Other {
        message: String,
    },
}

impl InspectorError {
    pub(crate) fn new(s: &str) -> Self {
        Self { message: s.into() }
    }
}

impl From<JsonError> for UpdateError {
    fn from(err: JsonError) -> Self {
        Self::Other {
            message: err.to_string().into(),
        }
    }
}
