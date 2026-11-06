//! The sound's pipe reader (Phase 9 plan, Task 3; `docs/spec/music.md`, Spectrum: analysis):
//! whole frames however the pipe splits them, none lost or doubled, samples out of range made
//! safe, a closed or replaced pipe heard as silence, stale sound as silence after 200 ms, and the
//! dev copy's override.

use std::fs::File;
use std::io::{ErrorKind, Write};
use std::os::fd::OwnedFd;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use odc_render::WINDOW;
use odc_service::audio::{AudioSource, Ended, PipeAudio, STALE, mono, sanitised};
use rustix::pipe::{PipeFlags, pipe_with};

/// How long something that must happen may take.
const WAIT: Duration = Duration::from_secs(3);

fn pipe() -> (OwnedFd, File) {
    let (read, write) = pipe_with(PipeFlags::CLOEXEC).expect("a pipe");
    (read, File::from(write))
}

/// One stereo frame, both channels `left` and `right`.
fn frame(left: f32, right: f32) -> Vec<u8> {
    let mut bytes = left.to_ne_bytes().to_vec();
    bytes.extend_from_slice(&right.to_ne_bytes());
    bytes
}

/// Frames whose mono samples are `samples`, both channels alike.
fn frames(samples: &[f32]) -> Vec<u8> {
    samples.iter().flat_map(|&s| frame(s, s)).collect()
}

/// Waits until `check` holds, at most [`WAIT`].
fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let until = Instant::now() + WAIT;
    while !check() {
        assert!(Instant::now() < until, "{what}");
        thread::sleep(Duration::from_millis(5));
    }
}

/// The window's last `n` samples.
fn tail(window: &[f32; WINDOW], n: usize) -> Vec<f32> {
    window[WINDOW - n..].to_vec()
}

/// Attaches a pipe for `holder`; returns its writer and what its end reports.
fn attach(audio: &PipeAudio, holder: &str) -> (File, Receiver<(String, Ended)>) {
    let (read, write) = pipe();
    let (tx, ended) = mpsc::channel();
    audio
        .attach(read, holder, move |who, why| {
            let _ = tx.send((who.to_owned(), why));
        })
        .expect("attached");
    (write, ended)
}

#[test]
fn a_frame_split_across_reads_is_joined_and_none_is_lost_or_doubled() {
    let audio = PipeAudio::new();
    let (mut writer, _ended) = attach(&audio, ":1.7");
    let samples = [0.25_f32, -0.5, 0.75];
    let bytes = frames(&samples);
    // Half a frame, a pause, then the rest.
    writer.write_all(&bytes[..4]).expect("written");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        tail(&audio.window(), 1),
        [0.0],
        "half a frame is no sample yet"
    );
    writer.write_all(&bytes[4..11]).expect("written");
    thread::sleep(Duration::from_millis(50));
    writer.write_all(&bytes[11..]).expect("written");
    eventually("the three samples", || tail(&audio.window(), 3) == samples);
    assert_eq!(tail(&audio.window(), 4), [0.0, 0.25, -0.5, 0.75]);
}

#[test]
fn a_whole_second_at_once_leaves_its_last_2048_samples() {
    let audio = PipeAudio::new();
    let (mut writer, _ended) = attach(&audio, ":1.7");
    let second: Vec<f32> = (0..48_000).map(|n| (n % 1000) as f32 / 1000.0).collect();
    let bytes = frames(&second);
    // 384 000 bytes: more than the pipe holds, so the writer waits for the reader.
    writer.write_all(&bytes).expect("written");
    let want: Vec<f32> = second[second.len() - WINDOW..].to_vec();
    eventually("the last 2048 samples", || audio.window().to_vec() == want);
}

#[test]
fn samples_out_of_range_reach_the_window_as_silence_or_full_scale() {
    for (sample, want) in [
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
        (f32::NEG_INFINITY, 0.0),
        (1e6, 1.0),
        (-1e6, -1.0),
        (0.5, 0.5),
    ] {
        assert_eq!(sanitised(sample), want, "{sample}");
    }
    // Each channel first, then the average.
    let bytes: [u8; 8] = frame(f32::NAN, 0.5).try_into().expect("a frame");
    assert_eq!(mono(bytes), 0.25);
    let audio = PipeAudio::new();
    let (mut writer, _ended) = attach(&audio, ":1.7");
    let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e6, -1e6];
    writer.write_all(&frames(&wild)).expect("written");
    eventually("the five samples", || {
        tail(&audio.window(), 5) == [0.0, 0.0, 0.0, 1.0, -1.0]
    });
}

#[test]
fn a_pipe_whose_writer_closes_is_let_go_and_heard_as_silence() {
    let audio = PipeAudio::new();
    let (mut writer, ended) = attach(&audio, ":1.7");
    writer.write_all(&frames(&[0.5; 100])).expect("written");
    eventually("the samples", || tail(&audio.window(), 1) == [0.5]);
    drop(writer);
    let (who, why) = ended.recv_timeout(WAIT).expect("its end is told");
    assert_eq!((who.as_str(), why), (":1.7", Ended::Closed));
    assert_eq!(audio.holder(), None);
    assert_eq!(audio.window(), [0.0; WINDOW]);
}

#[test]
fn a_newer_attach_replaces_the_older_whose_writer_then_gets_epipe() {
    let audio = PipeAudio::new();
    let (mut first, first_ended) = attach(&audio, ":1.7");
    first.write_all(&frames(&[0.25; 10])).expect("written");
    let (mut second, _ended) = attach(&audio, ":1.9");
    assert_eq!(audio.holder().map(|(_, h)| h), Some(":1.9".to_owned()));
    eventually(
        "the first pipe closed",
        || matches!(first.write_all(&frames(&[0.25])), Err(e) if e.kind() == ErrorKind::BrokenPipe),
    );
    assert!(
        first_ended
            .recv_timeout(Duration::from_millis(200))
            .is_err(),
        "a pipe closed on purpose isn't reported as ended"
    );
    second.write_all(&frames(&[-0.75; 3])).expect("written");
    eventually("the second pipe's samples", || {
        tail(&audio.window(), 3) == [-0.75; 3]
    });
}

#[test]
fn only_the_attachment_still_current_is_detached() {
    let audio = PipeAudio::new();
    let (_first, _a) = attach(&audio, ":1.7");
    let (old, _) = audio.holder().expect("attached");
    let (_second, _b) = attach(&audio, ":1.9");
    assert!(!audio.detach_if(old), "an older attachment");
    assert!(audio.holder().is_some());
    let (current, _) = audio.holder().expect("attached");
    assert!(audio.detach_if(current));
    assert_eq!(audio.holder(), None);
    assert_eq!(audio.detach(), None);
}

// music.md (owner, 2026-11-05): sound that stops coming is silence once its newest sample is
// older than 200 ms.
#[test]
fn sound_that_stops_coming_is_silence_after_200_ms() {
    let audio = PipeAudio::new();
    let (mut writer, _ended) = attach(&audio, ":1.7");
    writer.write_all(&frames(&[0.5; 64])).expect("written");
    eventually("the samples", || tail(&audio.window(), 1) == [0.5]);
    let read = Instant::now();
    assert_eq!(tail(&audio.window_at(read), 64), [0.5; 64]);
    assert_eq!(
        audio.window_at(read + STALE + Duration::from_millis(1)),
        [0.0; WINDOW]
    );
    // The pipe is still attached: new sound is heard again.
    writer.write_all(&frames(&[0.25; 8])).expect("written");
    eventually("the new samples", || tail(&audio.window(), 1) == [0.25]);
    assert!(audio.holder().is_some());
}

// docs/spec/dbus.md, Dev interface: the override's samples replace the attached pipe's until it's
// cleared, and its end of file is silence.
#[test]
fn an_override_replaces_the_attached_sound_until_cleared() {
    let audio = PipeAudio::new();
    let (mut helper, _ended) = attach(&audio, ":1.7");
    helper.write_all(&frames(&[0.25; 16])).expect("written");
    eventually("the helper's samples", || {
        tail(&audio.window(), 1) == [0.25]
    });
    let (read, write) = pipe();
    let mut test = write;
    audio.override_with(read).expect("overridden");
    assert!(audio.overridden());
    assert_eq!(
        audio.window(),
        [0.0; WINDOW],
        "the override's silence first"
    );
    test.write_all(&frames(&[-0.5; 16])).expect("written");
    eventually("the override's samples", || {
        tail(&audio.window(), 1) == [-0.5]
    });
    // The helper is still read, unheard.
    helper.write_all(&frames(&[0.75; 16])).expect("still read");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(tail(&audio.window(), 1), [-0.5]);
    drop(test);
    eventually("the override's end is silence", || {
        audio.window() == [0.0; WINDOW]
    });
    assert!(audio.overridden(), "until it's cleared");
    audio.clear_override();
    assert!(!audio.overridden());
    helper.write_all(&frames(&[0.125; 16])).expect("written");
    eventually("the helper's sound again", || {
        tail(&audio.window(), 1) == [0.125]
    });
}
