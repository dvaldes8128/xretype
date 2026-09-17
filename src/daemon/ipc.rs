use std::{
    collections::BTreeMap,
    env,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::actions::Action;

pub const PROTOCOL_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub version: u32,
    pub request: DaemonRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonRequest {
    Execute(Action),
    Run {
        workflow: String,
        parameters: BTreeMap<String, String>,
    },
    Reload,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub version: u32,
    pub result: std::result::Result<DaemonResponse, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Done,
    Status(DaemonStatus),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub version: String,
    pub pid: u32,
    pub config_generation: u64,
    pub workflow_count: usize,
    pub active_workflow: Option<String>,
    pub queue_depth: usize,
    pub active_overlay: Option<String>,
    pub last_reload_error: Option<String>,
}

pub fn socket_path() -> PathBuf {
    runtime_root().join("xretype/xretyped.sock")
}

pub(crate) fn runtime_root() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!("/tmp/xretype-runtime-{}", unsafe {
                libc::geteuid()
            }))
        })
}

pub fn call(request: DaemonRequest) -> Result<DaemonResponse> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).with_context(|| {
        format!(
            "could not connect to xretyped at {}; start the user service or use --standalone",
            path.display()
        )
    })?;
    write_envelope(
        &mut stream,
        &RequestEnvelope {
            version: PROTOCOL_VERSION,
            request,
        },
    )?;
    let response: ResponseEnvelope = read_json_line(&mut stream)?;
    if response.version != PROTOCOL_VERSION {
        bail!(
            "xretyped protocol version mismatch: client {}, server {}",
            PROTOCOL_VERSION,
            response.version
        );
    }
    response.result.map_err(anyhow::Error::msg)
}

pub(crate) fn read_request(stream: &mut UnixStream) -> Result<RequestEnvelope> {
    read_json_line(stream)
}

pub(crate) fn write_response(stream: &mut UnixStream, response: &ResponseEnvelope) -> Result<()> {
    write_envelope(stream, response)
}

fn write_envelope<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<()> {
    serde_json::to_writer(&mut *stream, value).context("could not encode daemon message")?;
    stream
        .write_all(b"\n")
        .context("could not write daemon message")?;
    stream.flush().context("could not flush daemon message")
}

fn read_json_line<T: for<'de> Deserialize<'de>>(stream: &mut UnixStream) -> Result<T> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream.take(MAX_MESSAGE_BYTES));
    let bytes = reader
        .read_line(&mut line)
        .context("could not read daemon message")?;
    if bytes == 0 {
        bail!("daemon closed the connection without a response");
    }
    if bytes as u64 >= MAX_MESSAGE_BYTES {
        bail!("daemon message exceeds the 1 MiB limit");
    }
    serde_json::from_str(&line).context("could not decode daemon message")
}

#[cfg(test)]
mod tests {
    use super::{DaemonRequest, PROTOCOL_VERSION, RequestEnvelope};

    #[test]
    fn protocol_round_trips_without_sensitive_debug_output_requirement() {
        let envelope = RequestEnvelope {
            version: PROTOCOL_VERSION,
            request: DaemonRequest::Run {
                workflow: "example".to_owned(),
                parameters: [("value".to_owned(), "secret".to_owned())]
                    .into_iter()
                    .collect(),
            },
        };
        let encoded = serde_json::to_string(&envelope).unwrap();
        let decoded: RequestEnvelope = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
    }
}
