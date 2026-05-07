/// Async ClamAV client using the clamd INSTREAM protocol over a Unix socket.
///
/// Protocol (RFC-style):
///   Client → `zINSTREAM\0`
///   Client → [4-byte big-endian chunk length][chunk bytes] … (repeat)
///   Client → [4 zero bytes]   ← signals end of stream
///   Server → `stream: OK\n`  |  `stream: {VirusName} FOUND\n`  |  `… ERROR\n`
use std::path::Path;

use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

use crate::error::AppError;

const CHUNK_SIZE: usize = 8 * 1024; // 8 KiB

#[derive(Debug, PartialEq)]
pub enum ScanOutcome {
    Clean,
    /// The name of the detected signature (e.g. `"Eicar-Signature"`).
    Infected(String),
}

/// Scans the file at `file_path` by streaming it to `clamd` over its Unix
/// socket at `socket_path`.  Fully async — no blocking I/O on the Tokio
/// executor.
pub async fn scan_file(socket_path: &str, file_path: &Path) -> Result<ScanOutcome, AppError> {
    // ── 1. Open socket ────────────────────────────────────────────────────────
    let mut sock = UnixStream::connect(socket_path).await.map_err(|e| {
        AppError::Internal(format!(
            "Cannot connect to clamd at '{socket_path}': {e}. Is clamd running?"
        ))
    })?;

    // ── 2. Send INSTREAM command (null-terminated variant) ────────────────────
    sock.write_all(b"zINSTREAM\0").await?;

    // ── 3. Stream file in fixed-size chunks ───────────────────────────────────
    let file = File::open(file_path).await?;
    let mut reader = BufReader::new(file);
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        // Prefix every chunk with its 4-byte big-endian length
        let len_prefix = (n as u32).to_be_bytes();
        sock.write_all(&len_prefix).await?;
        sock.write_all(&buf[..n]).await?;
    }

    // ── 4. Terminate stream with a zero-length chunk ──────────────────────────
    sock.write_all(&[0u8; 4]).await?;
    sock.flush().await?;

    // ── 5. Read clamd's verdict ───────────────────────────────────────────────
    //
    // We must shut down the write half so clamd doesn't wait for more data
    // before sending its response.
    sock.shutdown().await?;

    let mut response = String::new();
    sock.read_to_string(&mut response).await?;

    parse_response(response.trim())
}

/// Interprets the raw clamd INSTREAM response line.
///
/// Observed formats:
///   `stream: OK`
///   `stream: Eicar-Test-Signature FOUND`
///   `stream: Access denied. ERROR`
///   `INSTREAM size limit exceeded. ERROR`
fn parse_response(response: &str) -> Result<ScanOutcome, AppError> {
    if response.ends_with(" FOUND") {
        // Strip "stream: " prefix and " FOUND" suffix to isolate the virus name
        let virus = response
            .strip_prefix("stream: ")
            .unwrap_or(response)
            .strip_suffix(" FOUND")
            .unwrap_or(response)
            .trim()
            .to_string();
        return Ok(ScanOutcome::Infected(virus));
    }

    if response.ends_with("OK") {
        return Ok(ScanOutcome::Clean);
    }

    // Any other response is an operational error from clamd itself
    Err(AppError::Internal(format!("Unexpected clamd response: '{response}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean() {
        assert_eq!(parse_response("stream: OK").unwrap(), ScanOutcome::Clean);
    }

    #[test]
    fn parse_infected() {
        let result = parse_response("stream: Eicar-Test-Signature FOUND").unwrap();
        assert_eq!(result, ScanOutcome::Infected("Eicar-Test-Signature".into()));
    }

    #[test]
    fn parse_error_propagates() {
        assert!(parse_response("INSTREAM size limit exceeded. ERROR").is_err());
    }

    #[test]
    fn parse_virus_name_with_spaces() {
        let result = parse_response("stream: Win.Trojan.Agent-12345 FOUND").unwrap();
        assert_eq!(
            result,
            ScanOutcome::Infected("Win.Trojan.Agent-12345".into())
        );
    }
}
