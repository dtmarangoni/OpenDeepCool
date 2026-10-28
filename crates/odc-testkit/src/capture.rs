//! The capture-log reader. One message per line, `<unix ns> <pid> <tid> <dir> <len> <hex>`: the
//! format of the research logger's captures, which the recording wrapper writes too (Phase 5
//! plan, Tasks 5 and 6). A malformed line panics, naming its file and line.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Written to the device.
    Out,
    /// Read from the device.
    In,
}

/// One logged message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Captured {
    /// The line it was on, from 1.
    pub line: usize,
    pub unix_ns: u64,
    pub dir: Direction,
    pub bytes: Vec<u8>,
}

/// Reads a capture log, one message per line, as it goes.
///
/// # Panics
///
/// When the file can't be opened or read, or a line is malformed, naming the file and line.
pub fn read(path: impl AsRef<Path>) -> impl Iterator<Item = Captured> {
    let path = path.as_ref().to_path_buf();
    let file = File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    BufReader::new(file)
        .lines()
        .enumerate()
        .map(move |(i, text)| {
            let line = i + 1;
            let text = text.unwrap_or_else(|e| panic!("{}:{line}: {e}", path.display()));
            let (unix_ns, dir, bytes) =
                parse_line(&text).unwrap_or_else(|e| panic!("{}:{line}: {e}", path.display()));
            Captured {
                line,
                unix_ns,
                dir,
                bytes,
            }
        })
}

fn parse_line(text: &str) -> Result<(u64, Direction, Vec<u8>), String> {
    let fields: Vec<&str> = text.split(' ').collect();
    let [ns, pid, tid, dir, len, hex] = fields.as_slice() else {
        return Err(format!("{} fields, not 6: {text:?}", fields.len()));
    };
    let unix_ns = ns
        .parse::<u64>()
        .map_err(|_| format!("{ns:?} isn't a time in ns"))?;
    for (name, id) in [("pid", pid), ("tid", tid)] {
        id.parse::<u32>()
            .map_err(|_| format!("{id:?} isn't a {name}"))?;
    }
    let dir = match *dir {
        "out" => Direction::Out,
        "in" => Direction::In,
        _ => return Err(format!("{dir:?} is neither `out` nor `in`")),
    };
    let len = len
        .parse::<usize>()
        .map_err(|_| format!("{len:?} isn't a length"))?;
    if hex.len() != 2 * len || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{len} bytes need {} hex digits", 2 * len));
    }
    let bytes = (0..len)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("two hex digits"))
        .collect();
    Ok((unix_ns, dir, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logged_write_reads_back() {
        let (ns, dir, bytes) =
            parse_line("1792643862902677347 236805 236808 out 3 10680a").expect("a good line");
        assert_eq!(ns, 1_792_643_862_902_677_347);
        assert_eq!(dir, Direction::Out);
        assert_eq!(bytes, [0x10, 0x68, 0x0a]);
        assert_eq!(
            parse_line("1 0 0 in 1 FF").expect("a read").1,
            Direction::In
        );
    }

    #[test]
    fn malformed_lines_are_refused() {
        let refused = [
            ("1 0 0 out 1", "5 fields, not 6"),
            ("x 0 0 out 1 10", "isn't a time in ns"),
            ("1 p 0 out 1 10", "isn't a pid"),
            ("1 0 0 sideways 1 10", "is neither `out` nor `in`"),
            ("1 0 0 out 2 10", "2 bytes need 4 hex digits"),
            ("1 0 0 out 1 +1", "1 bytes need 2 hex digits"),
            ("1 0 0 out 1 1g", "1 bytes need 2 hex digits"),
        ];
        for (text, reason) in refused {
            let error = parse_line(text).expect_err(text);
            assert!(error.contains(reason), "{text:?}: {error}");
        }
    }
}
