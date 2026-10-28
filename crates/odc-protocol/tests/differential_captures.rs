//! Layer 2, local (Phase 5 plan, Task 5): the encoder reproduces, byte for byte, every message
//! DeepCreative was captured sending; the owner checked those pictures on the cooler in Phase 3.
//! The captures are git-ignored, so this runs only on the owner's PC, with `--ignored`. A missing
//! capture fails it, never skips it.
//!
//! `cargo test -p odc-protocol --test differential_captures -- --ignored --nocapture`

use std::path::Path;

use odc_protocol::{Message, decode, encode_frame, startup_report};
use odc_testkit::capture::{self, Direction};

const LOGS: [&str; 2] = ["lp360-hidraw.log", "reconnect-20261024.log"];

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_message_re_encodes_byte_for_byte() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research/logs");
    let (mut messages, mut startups, mut frames) = (0, 0, 0);
    let mut mismatches: Vec<String> = Vec::new();
    for name in LOGS {
        let path = dir.join(name);
        assert!(
            path.is_file(),
            "{} is missing; this test needs the local captures",
            path.display()
        );
        for captured in capture::read(&path) {
            let at = format!("{name}:{}", captured.line);
            if captured.dir != Direction::Out {
                mismatches.push(format!(
                    "{at}: a read, but DeepCreative never reads (protocol.md)"
                ));
                continue;
            }
            let Ok(bytes) = <[u8; 64]>::try_from(captured.bytes.as_slice()) else {
                mismatches.push(format!("{at}: {} bytes, not 64", captured.bytes.len()));
                continue;
            };
            messages += 1;
            let again = match decode(&bytes) {
                Ok(Message::Startup) => {
                    startups += 1;
                    startup_report()
                }
                Ok(Message::Frame(frame)) => {
                    frames += 1;
                    encode_frame(&frame)
                }
                Err(e) => {
                    mismatches.push(format!("{at}: {e}"));
                    continue;
                }
            };
            if again.as_bytes() != &bytes {
                mismatches.push(format!("{at}: re-encoded to different bytes"));
            }
        }
    }
    println!(
        "{messages} messages: {startups} start-ups, {frames} frames; {} mismatches",
        mismatches.len()
    );
    assert!(
        mismatches.is_empty(),
        "{} mismatches; the first: {:#?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(5)]
    );
    assert!(messages > 0, "the captures hold no message");
}
