//! `sensors-detect.json`: what one detection run created, and for which board (`docs/spec/
//! sensors.md`, Board sensor driver). It holds fixed fields only, yes/no facts and never a path,
//! so the removal script acts on its two fixed paths alone (owner, 2026-10-26).

use std::time::{SystemTime, UNIX_EPOCH};

/// The record's file name in the detection's own state directory.
pub const RECORD: &str = "sensors-detect.json";

/// The board, as DMI names it (`/sys/class/dmi/id/board_vendor`, `board_name`, `board_version`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

/// One run's record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub board: Board,
    /// The run's date, `YYYY-MM-DD` in UTC.
    pub date: String,
    /// Whether this run created `/etc/conf.d/lm_sensors`.
    pub created_conf_d: bool,
    /// Whether this run created `/etc/modprobe.d/lm_sensors.conf`.
    pub created_modprobe_d: bool,
    /// Whether `lm_sensors.service` was enabled before this run.
    pub lm_sensors_enabled_before: bool,
}

/// The keys, in the order the file holds them.
const KEYS: [&str; 8] = [
    "version",
    "board_vendor",
    "board_name",
    "board_version",
    "date",
    "created_conf_d",
    "created_modprobe_d",
    "lm_sensors_enabled_before",
];

impl Record {
    /// The file's text: version 1, one key per line, in a fixed order.
    pub fn render(&self) -> String {
        let values = [
            "1".to_owned(),
            quote(&self.board.vendor),
            quote(&self.board.name),
            quote(&self.board.version),
            quote(&self.date),
            self.created_conf_d.to_string(),
            self.created_modprobe_d.to_string(),
            self.lm_sensors_enabled_before.to_string(),
        ];
        let lines: Vec<String> = KEYS
            .iter()
            .zip(values)
            .map(|(key, value)| format!("  \"{key}\": {value}"))
            .collect();
        format!("{{\n{}\n}}\n", lines.join(",\n"))
    }

    /// The record in `text`, read strictly: exactly the layout [`Record::render`] writes.
    pub fn parse(text: &str) -> Result<Record, String> {
        let lines: Vec<&str> = text
            .strip_suffix('\n')
            .unwrap_or(text)
            .split('\n')
            .collect();
        if lines.len() != KEYS.len() + 2 || lines[0] != "{" || lines[lines.len() - 1] != "}" {
            return Err("not the record's layout".into());
        }
        let mut values = Vec::new();
        for (i, (line, key)) in lines[1..=KEYS.len()].iter().zip(KEYS).enumerate() {
            let line = if i + 1 < KEYS.len() {
                line.strip_suffix(',')
                    .ok_or_else(|| format!("{key}: no comma"))?
            } else {
                line
            };
            let value = line
                .strip_prefix(&format!("  \"{key}\": "))
                .ok_or_else(|| format!("line {} isn't {key:?}", i + 2))?;
            values.push(value);
        }
        if values[0] != "1" {
            return Err("not version 1".into());
        }
        let text = |value: &str| unquote(value);
        let flag = |value: &str, key: &str| match value {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("{key} isn't true or false")),
        };
        let date = text(values[4])?;
        if !is_date(&date) {
            return Err(format!("{date:?} isn't a date"));
        }
        Ok(Record {
            board: Board {
                vendor: text(values[1])?,
                name: text(values[2])?,
                version: text(values[3])?,
            },
            date,
            created_conf_d: flag(values[5], KEYS[5])?,
            created_modprobe_d: flag(values[6], KEYS[6])?,
            lm_sensors_enabled_before: flag(values[7], KEYS[7])?,
        })
    }
}

/// `text` as a JSON string: quotes, backslashes and control characters escaped.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The text of a JSON string as [`quote`] writes it, and the escapes JSON allows.
fn unquote(value: &str) -> Result<String, String> {
    let inner = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(|| format!("{value:?} isn't a string"))?;
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let escaped = match chars.next() {
                    Some('"') => '"',
                    Some('\\') => '\\',
                    Some('/') => '/',
                    Some('b') => '\u{8}',
                    Some('f') => '\u{c}',
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some('u') => {
                        let hex: String = chars.by_ref().take(4).collect();
                        let code = u32::from_str_radix(&hex, 16)
                            .ok()
                            .filter(|_| hex.len() == 4)
                            .ok_or_else(|| format!("{value:?}: a bad \\u escape"))?;
                        char::from_u32(code)
                            .ok_or_else(|| format!("{value:?}: a \\u escape of no character"))?
                    }
                    _ => return Err(format!("{value:?}: a bad escape")),
                };
                out.push(escaped);
            }
            '"' => return Err(format!("{value:?}: a quote inside")),
            c if u32::from(c) < 0x20 => return Err(format!("{value:?}: a control character")),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// Whether `text` is `YYYY-MM-DD` in digits.
fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// Today's date in UTC, `YYYY-MM-DD`, from the days since 1970-01-01 (the civil calendar
/// algorithm in Howard Hinnant's "chrono-Compatible Low-Level Date Algorithms").
pub fn today() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    civil(days)
}

fn civil(days: u64) -> String {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Record {
        Record {
            board: Board {
                vendor: "Made-up \"Boards\" \\ Co.\t".to_owned(),
                name: "FAKE-1\u{1}".to_owned(),
                version: "1.0 ".to_owned(),
            },
            date: "2026-10-29".to_owned(),
            created_conf_d: true,
            created_modprobe_d: false,
            lm_sensors_enabled_before: false,
        }
    }

    #[test]
    fn a_record_reads_back_as_written() {
        let record = record();
        assert_eq!(Record::parse(&record.render()), Ok(record));
    }

    #[test]
    fn anything_but_the_written_layout_is_refused() {
        let good = record().render();
        for bad in [
            String::new(),
            good.replace("\"version\": 1", "\"version\": 2"),
            good.replace("\"created_conf_d\": true", "\"created_conf_d\": yes"),
            good.replace("2026-10-29", "29/10/2026"),
            good.replace("  \"board_name\"", "  \"path\""),
            good.replace(",\n  \"date\"", "\n  \"date\""),
            good.replace("}\n", "  \"extra\": 1\n}\n"),
            good.replace("\\\"Boards\\\"", "\"Boards\""),
        ] {
            assert!(Record::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn days_since_1970_give_the_civil_date() {
        assert_eq!(civil(0), "1970-01-01");
        assert_eq!(civil(11_016), "2000-02-29");
        assert_eq!(civil(20_755), "2026-10-29");
    }
}
