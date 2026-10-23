//! Opt-in development protocol and deterministic comparison helpers.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
pub const PROTOCOL: &str = "rust-ui.devctl/0.1";
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
pub const MAX_PIXELS: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub token: String,
    pub id: u64,
    pub command: String,
    pub arguments: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}
impl Error {
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            details: None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub protocol: String,
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}
impl Response {
    pub fn from_result(id: u64, r: Result<Value>) -> Self {
        match r {
            Ok(v) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: true,
                result: Some(v),
                error: None,
            },
            Err(e) => Self {
                protocol: PROTOCOL.into(),
                id,
                ok: false,
                result: None,
                error: Some(e),
            },
        }
    }
}
pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(v).map_err(|e| Error::new("PROTOCOL_ERROR", e))?;
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    let mut out = (bytes.len() as u32).to_le_bytes().to_vec();
    out.extend(bytes);
    Ok(out)
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "4 MiB limit"));
    }
    serde_json::from_slice(bytes).map_err(|e| Error::new("PROTOCOL_ERROR", e))
}
pub fn read_frame<R: Read, T: serde::de::DeserializeOwned>(r: &mut R) -> Result<T> {
    let mut n = [0; 4];
    r.read_exact(&mut n).map_err(ioerr)?;
    let n = u32::from_le_bytes(n) as usize;
    if n > MAX_MESSAGE {
        return Err(Error::new("MESSAGE_TOO_LARGE", "frame exceeds 4 MiB"));
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b).map_err(ioerr)?;
    decode(&b)
}
pub fn ioerr(e: impl ToString) -> Error {
    Error::new("IO_ERROR", e)
}

pub mod compare;
pub mod contract;
pub mod geom;
pub mod image;
pub mod model;

pub use compare::*;
pub use contract::*;
pub use geom::*;
pub use image::*;
pub use model::*;

#[cfg(windows)]
#[path = "../platform/win32/devctl_client.rs"]
pub mod client;

#[cfg(test)]
mod tests {
    include!("tests_1.rs");
    include!("tests_2.rs");
    include!("tests_3.rs");
}
