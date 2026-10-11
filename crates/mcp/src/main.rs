//! `drapewright-mcp`: Drapewright's MCP server, for AI assistants such as
//! Claude. It reads JSON-RPC messages on stdin, one per line, and answers
//! on stdout. See `docs/ai-assistants.md`.

use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use drapewright_mcp::{Host, Server};

const HELP: &str = "drapewright-mcp: Drapewright's MCP server, for AI assistants.

It talks the Model Context Protocol over stdin and stdout, so an assistant
starts it; you don't run it by hand. To add it to Claude Code:

    claude mcp add drapewright -- /path/to/drapewright-mcp

See docs/ai-assistants.md for Claude Desktop and other assistants.";

/// The time now as RFC 3339 in UTC, e.g. "2026-10-11T09:15:47Z".
fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// A random version-4 UUID, as the app gives new projects.
fn new_id() -> String {
    let mut bytes = [0u8; 16];
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_usize(i);
        h.write_u32(std::process::id());
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn main() {
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" | "-V" => println!("drapewright-mcp {}", drapewright_pattern_core::VERSION),
            _ => println!("{HELP}"),
        }
        return;
    }
    let mut server = Server::new(Host { now, new_id });
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if let Some(reply) = server.handle_line(&line)
            && writeln!(stdout, "{reply}")
                .and_then(|()| stdout.flush())
                .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_ids_look_right() {
        let t = now();
        assert_eq!(t.len(), 20);
        assert!(t.starts_with("20") && t.ends_with('Z'), "{t}");
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(new_id(), id);
    }
}
