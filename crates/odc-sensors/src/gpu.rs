//! The GPUs the display can read, their kinds, and which one feeds the display
//! (`docs/spec/sensors.md`, GPU choice; `docs/spec/settings.md`, GPU list order).

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::str::FromStr;

use crate::json::{self, Json};

/// The ranking file's name in the service's state directory, written by `odc-gpu-rank`.
pub const RANK_FILE: &str = "gpu-rank.json";

/// A bigger ranking file is refused as broken.
const MAX_RANK_FILE: u64 = 64 * 1024;

/// A PCI address as sysfs writes it, `domain:bus:device.function` in lower-case hexadecimal, for
/// example `0000:0a:00.0`. GPUs are told apart by it, never by DRM card number, which can change
/// between boots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PciAddr {
    domain: u32,
    bus: u8,
    device: u8,
    function: u8,
}

impl FromStr for PciAddr {
    type Err = String;

    fn from_str(text: &str) -> Result<PciAddr, String> {
        let bad = || format!("{text:?} isn't a PCI address");
        let hex = |part: &str, digits: std::ops::RangeInclusive<usize>| {
            let ok = digits.contains(&part.len())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            ok.then(|| u32::from_str_radix(part, 16).ok()).flatten()
        };
        let (domain, rest) = text.split_once(':').ok_or_else(bad)?;
        let (bus, rest) = rest.split_once(':').ok_or_else(bad)?;
        let (device, function) = rest.split_once('.').ok_or_else(bad)?;
        let byte = |n: Option<u32>, below: u32| {
            n.filter(|n| *n < below).and_then(|n| u8::try_from(n).ok())
        };
        Ok(PciAddr {
            domain: hex(domain, 4..=8).ok_or_else(bad)?,
            bus: byte(hex(bus, 2..=2), 0x100).ok_or_else(bad)?,
            device: byte(hex(device, 2..=2), 0x20).ok_or_else(bad)?,
            function: byte(hex(function, 1..=1), 8).ok_or_else(bad)?,
        })
    }
}

impl fmt::Display for PciAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{:x}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

/// A GPU's kind: a graphics card, part of the CPU, or unknown, with why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuKind {
    Dedicated,
    Integrated,
    Unknown(Unknown),
}

/// Why a GPU's kind is unknown. Every unknown GPU ranks after an integrated one (owner,
/// 2026-10-27; solution review P6-07).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unknown {
    /// Its driver has no kind signal, such as nouveau.
    NoSignal,
    /// The query that tells its kind failed, with the error; logged and named in the GPU list.
    QueryFailed { error: String },
    /// No ranking lists it: there's none yet, or the GPU came after it.
    NotRanked,
}

impl GpuKind {
    /// Dedicated first, then integrated, then unknown.
    fn rank(&self) -> u8 {
        match self {
            GpuKind::Dedicated => 0,
            GpuKind::Integrated => 1,
            GpuKind::Unknown(_) => 2,
        }
    }
}

/// A GPU the display can read: a PCI display controller with a driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpu {
    pub pci: PciAddr,
    pub driver: String,
    pub kind: GpuKind,
}

/// What `odc-gpu-rank` found for one GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RankedKind {
    /// Dedicated, and the driver's signal that said so.
    Dedicated {
        signal: String,
    },
    Integrated {
        signal: String,
    },
    NoSignal,
    QueryFailed {
        error: String,
    },
}

impl From<RankedKind> for GpuKind {
    fn from(kind: RankedKind) -> GpuKind {
        match kind {
            RankedKind::Dedicated { .. } => GpuKind::Dedicated,
            RankedKind::Integrated { .. } => GpuKind::Integrated,
            RankedKind::NoSignal => GpuKind::Unknown(Unknown::NoSignal),
            RankedKind::QueryFailed { error } => GpuKind::Unknown(Unknown::QueryFailed { error }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranked {
    pub pci: PciAddr,
    pub driver: String,
    pub kind: RankedKind,
}

/// The ranking `odc-gpu-rank` writes into `gpu-rank.json`: every GPU it found, by PCI address,
/// with its kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ranking {
    pub gpus: Vec<Ranked>,
}

impl Ranking {
    /// The file's text: version 1 and one object per GPU.
    pub fn render(&self) -> String {
        let gpus: Vec<String> = self
            .gpus
            .iter()
            .map(|gpu| {
                let mut fields = vec![
                    format!("\"pci\": {}", json::quote(&gpu.pci.to_string())),
                    format!("\"driver\": {}", json::quote(&gpu.driver)),
                ];
                match &gpu.kind {
                    RankedKind::Dedicated { signal } => fields.extend([
                        "\"kind\": \"dedicated\"".to_owned(),
                        format!("\"signal\": {}", json::quote(signal)),
                    ]),
                    RankedKind::Integrated { signal } => fields.extend([
                        "\"kind\": \"integrated\"".to_owned(),
                        format!("\"signal\": {}", json::quote(signal)),
                    ]),
                    RankedKind::NoSignal => fields.extend([
                        "\"kind\": \"unknown\"".to_owned(),
                        "\"why\": \"no-signal\"".to_owned(),
                    ]),
                    RankedKind::QueryFailed { error } => fields.extend([
                        "\"kind\": \"unknown\"".to_owned(),
                        "\"why\": \"query-failed\"".to_owned(),
                        format!("\"error\": {}", json::quote(error)),
                    ]),
                }
                format!("    {{{}}}", fields.join(", "))
            })
            .collect();
        if gpus.is_empty() {
            "{\n  \"version\": 1,\n  \"gpus\": []\n}\n".to_owned()
        } else {
            format!(
                "{{\n  \"version\": 1,\n  \"gpus\": [\n{}\n  ]\n}}\n",
                gpus.join(",\n")
            )
        }
    }

    /// The ranking in `text`, read strictly: version 1, each GPU with a well-formed address, its
    /// driver and a known kind with the fields that kind has and no others, and no address twice.
    pub fn parse(text: &str) -> Result<Ranking, String> {
        let Json::Object(members) = json::parse(text)? else {
            return Err("not an object".into());
        };
        let (mut version, mut list) = (None, None);
        for (key, value) in members {
            match key.as_str() {
                "version" => version = Some(value),
                "gpus" => list = Some(value),
                other => return Err(format!("an unknown key {other:?}")),
            }
        }
        if version != Some(Json::Number("1".into())) {
            return Err("not version 1".into());
        }
        let Some(Json::Array(items)) = list else {
            return Err("no list of GPUs".into());
        };
        let mut gpus: Vec<Ranked> = Vec::new();
        for item in items {
            let gpu = ranked(item)?;
            if gpus.iter().any(|g| g.pci == gpu.pci) {
                return Err(format!("{} twice", gpu.pci));
            }
            gpus.push(gpu);
        }
        Ok(Ranking { gpus })
    }
}

/// One GPU of the file.
fn ranked(item: Json) -> Result<Ranked, String> {
    let Json::Object(members) = item else {
        return Err("a GPU that isn't an object".into());
    };
    let mut fields = Vec::new();
    for (key, value) in members {
        if !["pci", "driver", "kind", "signal", "why", "error"].contains(&key.as_str()) {
            return Err(format!("an unknown key {key:?}"));
        }
        let Json::String(text) = value else {
            return Err(format!("{key:?} isn't text"));
        };
        fields.push((key, text));
    }
    let mut take = |key: &str| {
        let i = fields.iter().position(|(k, _)| k == key)?;
        Some(fields.swap_remove(i).1)
    };
    let pci: PciAddr = take("pci").ok_or("a GPU without its address")?.parse()?;
    let driver = take("driver")
        .filter(|d| !d.is_empty() && !d.chars().any(char::is_control))
        .ok_or_else(|| format!("{pci}: no driver"))?;
    let kind = match take("kind").as_deref() {
        Some("dedicated") => RankedKind::Dedicated {
            signal: take("signal").ok_or_else(|| format!("{pci}: no signal"))?,
        },
        Some("integrated") => RankedKind::Integrated {
            signal: take("signal").ok_or_else(|| format!("{pci}: no signal"))?,
        },
        Some("unknown") => match take("why").as_deref() {
            Some("no-signal") => RankedKind::NoSignal,
            Some("query-failed") => RankedKind::QueryFailed {
                error: take("error").ok_or_else(|| format!("{pci}: no error"))?,
            },
            _ => return Err(format!("{pci}: unknown, but not why")),
        },
        _ => return Err(format!("{pci}: an unknown kind")),
    };
    if let Some((key, _)) = fields.first() {
        return Err(format!("{pci}: {key:?} doesn't belong to its kind"));
    }
    Ok(Ranked { pci, driver, kind })
}

/// Whether a device folder's `class` file says base class 0x03, a display controller (the
/// kernel's `pci_ids.h`, `PCI_BASE_CLASS_DISPLAY`).
pub(crate) fn is_display(dir: &Path) -> bool {
    fs::read_to_string(dir.join("class"))
        .ok()
        .and_then(|class| u32::from_str_radix(class.trim().trim_start_matches("0x"), 16).ok())
        .is_some_and(|class| class >> 16 == 0x03)
}

/// Every PCI display controller with a driver, as (address, driver), by address.
pub fn display_devices(sys: &Path) -> Vec<(PciAddr, String)> {
    let Ok(entries) = fs::read_dir(sys.join("bus/pci/devices")) else {
        return Vec::new();
    };
    let mut devices: Vec<(PciAddr, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let pci: PciAddr = entry.file_name().to_str()?.parse().ok()?;
            let dir = entry.path();
            if !is_display(&dir) {
                return None;
            }
            let driver = fs::read_link(dir.join("driver")).ok()?;
            Some((pci, driver.file_name()?.to_str()?.to_owned()))
        })
        .collect();
    devices.sort();
    devices
}

/// The GPUs in the order the settings app lists them: dedicated, integrated, then unknown, each
/// by PCI address. Each kind comes from `ranking` when it lists the GPU.
pub fn gpus(sys: &Path, ranking: Option<&Ranking>) -> Vec<Gpu> {
    let mut gpus: Vec<Gpu> = display_devices(sys)
        .into_iter()
        .map(|(pci, driver)| {
            let kind = ranking
                .and_then(|r| r.gpus.iter().find(|g| g.pci == pci))
                .map_or(GpuKind::Unknown(Unknown::NotRanked), |g| {
                    g.kind.clone().into()
                });
            Gpu { pci, driver, kind }
        })
        .collect();
    gpus.sort_by_key(|gpu| (gpu.kind.rank(), gpu.pci));
    gpus
}

/// The GPU the display reads: the saved one when it's there; otherwise the best ranked, while the
/// saved choice is kept for when its GPU is back (`docs/spec/sensors.md`, GPU choice).
pub fn choose(gpus: &[Gpu], saved: Option<PciAddr>) -> Option<&Gpu> {
    saved
        .and_then(|pci| gpus.iter().find(|gpu| gpu.pci == pci))
        .or_else(|| gpus.iter().min_by_key(|gpu| (gpu.kind.rank(), gpu.pci)))
}

/// Whether the ranking must be done again: the GPUs differ from those it lists, and the user
/// hasn't chosen one (owner, 2026-10-24). Once the user has chosen, a new GPU stays unknown.
pub fn rank_is_stale(gpus: &[Gpu], ranking: &Ranking, user_has_chosen: bool) -> bool {
    let present: BTreeSet<PciAddr> = gpus.iter().map(|gpu| gpu.pci).collect();
    let ranked: BTreeSet<PciAddr> = ranking.gpus.iter().map(|gpu| gpu.pci).collect();
    present != ranked && !user_has_chosen
}

/// What the service made of the ranking file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    /// The ranking to take kinds from; `None` when there was none or it was broken.
    pub ranking: Option<Ranking>,
    /// When the file was deleted, why, for the journal.
    pub deleted: Option<String>,
}

/// What the service does with the ranking file at start-up: it reads it, deletes a broken one
/// whether or not the user has chosen (overview review's cross-check, 2026-10-27), and deletes a
/// stale one while the user hasn't (owner, 2026-10-24), so the helper ranks again at the next
/// start. A stale ranking still gives the kinds of the GPUs it lists until then.
pub fn review_ranking(sys: &Path, path: &Path, user_has_chosen: bool) -> Review {
    let ranking = match read_ranking(path) {
        Ok(None) => {
            return Review {
                ranking: None,
                deleted: None,
            };
        }
        Ok(Some(ranking)) => ranking,
        Err(why) => {
            return Review {
                ranking: None,
                deleted: Some(delete(path, &format!("broken, {why}"))),
            };
        }
    };
    let deleted = rank_is_stale(&gpus(sys, Some(&ranking)), &ranking, user_has_chosen)
        .then(|| delete(path, "the GPUs changed since it was written"));
    Review {
        ranking: Some(ranking),
        deleted,
    }
}

/// The ranking file, `None` when there's none. A link, anything but a plain file, or one too big
/// is refused.
fn read_ranking(path: &Path) -> Result<Option<Ranking>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() {
        return Err("not a plain file".into());
    }
    if meta.len() > MAX_RANK_FILE {
        return Err(format!("{} bytes, more than {MAX_RANK_FILE}", meta.len()));
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ranking::parse(&text).map(Some)
}

/// Deletes the ranking file and says so; a link is removed, never what it points to.
fn delete(path: &Path, why: &str) -> String {
    match fs::remove_file(path) {
        Ok(()) => format!(
            "{}: {why}; deleted, so the GPUs are ranked again at the next start",
            path.display()
        ),
        Err(e) => format!("{}: {why}; couldn't delete it: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_addresses_read_and_print_as_sysfs_writes_them() {
        for text in ["0000:0a:00.0", "0000:00:02.0", "10000:e0:1f.7"] {
            let pci: PciAddr = text.parse().expect(text);
            assert_eq!(pci.to_string(), text);
        }
        for bad in [
            "",
            "0000:0a:00",
            "0000:0A:00.0",
            "000:0a:00.0",
            "0000:0a:20.0",
            "0000:0a:00.8",
            "0000:0a:000.0",
            "0000:0g:00.0",
            "0000:0a:00.0 ",
            "x0000:0a:00.0",
        ] {
            assert!(bad.parse::<PciAddr>().is_err(), "{bad:?}");
        }
        let low: PciAddr = "0000:00:02.0".parse().expect("an address");
        let high: PciAddr = "0000:0a:00.0".parse().expect("an address");
        assert!(low < high);
    }
}
