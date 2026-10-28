//! The 64-byte HID output reports the LP360 accepts, and the decoder that reads them back
//! (`docs/spec/protocol.md`: Message layout, Start-up command, Frame).

use std::fmt;

use crate::Frame;

const LEN: usize = 64;
const REPORT_ID: u8 = 0x10;
const HEADER: [u8; 3] = [0x68, 0x01, 0x05];
const TERMINATOR: u8 = 0x16;
const STARTUP: [u8; 2] = [0x03, 0x08];
const FRAME_TAG: u8 = 0x01;
/// The frame payload: the tag `01` and 28 pixel bytes.
const FRAME_PAYLOAD: usize = 29;
/// The bit of each row pair, rows 1–2 to 13–14, in every pixel byte. Bit `0x08` is never used.
const PAIR_BITS: [u8; 7] = [0x10, 0x20, 0x40, 0x80, 0x01, 0x02, 0x04];
const UNUSED_BIT: u8 = 0x08;

/// One 64-byte HID output report, ready to write: report ID 16 and 63 bytes. Only
/// [`startup_report`] and [`encode_frame`] make one, so only the messages the spec documents can
/// reach the cooler (AGENTS.md rule 9). Raw bytes can't become a report:
///
/// ```compile_fail
/// let forged = odc_protocol::Report([0x10; 64]); // the field is private
/// ```
///
/// ```compile_fail
/// let forged: odc_protocol::Report = [0x10; 64].into(); // no conversion from bytes
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Report([u8; LEN]);

impl Report {
    /// The header, the payload, its checksum, the terminator, and zeros.
    fn with_payload(payload: &[u8]) -> Self {
        let n = payload.len();
        let mut bytes = [0; LEN];
        bytes[0] = REPORT_ID;
        bytes[1..4].copy_from_slice(&HEADER);
        bytes[4] = u8::try_from(n).expect("a payload fits the report");
        bytes[5..5 + n].copy_from_slice(payload);
        bytes[5 + n] = checksum(&bytes[1..5 + n]);
        bytes[6 + n] = TERMINATOR;
        Report(bytes)
    }

    /// The 64 bytes to write, report ID first.
    pub fn as_bytes(&self) -> &[u8; LEN] {
        &self.0
    }
}

impl fmt::Debug for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Report(")?;
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}

/// The sum of bytes 1 … 4 + N, modulo 256.
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |sum: u8, &b| sum.wrapping_add(b))
}

/// The start-up command `03 08`, sent before the first frame of every connection.
pub fn startup_report() -> Report {
    Report::with_payload(&STARTUP)
}

/// Pixel (r, c), with pair k = (r + 1) div 2, is bit k of pixel byte c for an odd row and of
/// pixel byte 29 − c for an even one.
fn pixel_byte(row: usize, col: usize) -> usize {
    if row % 2 == 1 { col } else { 29 - col }
}

/// The report that shows `frame`: payload `01` and 28 pixel bytes.
pub fn encode_frame(frame: &Frame) -> Report {
    let mut payload = [0; FRAME_PAYLOAD];
    payload[0] = FRAME_TAG;
    for row in 1..=Frame::SIZE {
        // Pair k = (row + 1) div 2, from 1.
        let bit = PAIR_BITS[row.div_ceil(2) - 1];
        for col in 1..=Frame::SIZE {
            if frame.get(row, col) {
                // Pixel byte b is payload byte b + 1, payload[b].
                payload[pixel_byte(row, col)] |= bit;
            }
        }
    }
    Report::with_payload(&payload)
}

/// A report read back: one of the two messages the spec documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Startup,
    Frame(Frame),
}

/// Why 64 bytes aren't a documented message. Each region of the layout has its own reason, so a
/// failing capture line says what is wrong with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Byte 0 isn't report ID 16.
    ReportId(u8),
    /// Bytes 1–3 aren't `68 01 05`.
    Header([u8; 3]),
    /// Byte 4, the payload length, belongs to no documented message.
    Length(u8),
    /// The checksum byte doesn't match the sum of bytes 1 … 4 + N.
    Checksum { expected: u8, found: u8 },
    /// The byte after the checksum isn't `0x16`.
    Terminator(u8),
    /// A padding byte, at this index, isn't zero.
    Padding(usize),
    /// The payload is neither the start-up command nor a frame.
    UnknownPayload,
    /// A frame's pixel byte, numbered from 1, has bit `0x08` set.
    UnusedBit(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::ReportId(id) => write!(f, "report ID {id:#04x}, not 0x10"),
            DecodeError::Header(h) => {
                write!(
                    f,
                    "header {:02x} {:02x} {:02x}, not 68 01 05",
                    h[0], h[1], h[2]
                )
            }
            DecodeError::Length(n) => write!(f, "payload length {n}, not 2 or 29"),
            DecodeError::Checksum { expected, found } => {
                write!(f, "checksum {found:#04x}, expected {expected:#04x}")
            }
            DecodeError::Terminator(t) => write!(f, "terminator {t:#04x}, not 0x16"),
            DecodeError::Padding(i) => write!(f, "padding byte {i} isn't zero"),
            DecodeError::UnknownPayload => write!(f, "neither the start-up command nor a frame"),
            DecodeError::UnusedBit(b) => write!(f, "pixel byte {b} has bit 0x08 set"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Reads 64 bytes back into a message, refusing anything the spec doesn't document. It never
/// yields a [`Report`].
pub fn decode(bytes: &[u8; LEN]) -> Result<Message, DecodeError> {
    if bytes[0] != REPORT_ID {
        return Err(DecodeError::ReportId(bytes[0]));
    }
    let header = [bytes[1], bytes[2], bytes[3]];
    if header != HEADER {
        return Err(DecodeError::Header(header));
    }
    let n = bytes[4];
    let len = usize::from(n);
    if len != STARTUP.len() && len != FRAME_PAYLOAD {
        return Err(DecodeError::Length(n));
    }
    let expected = checksum(&bytes[1..5 + len]);
    if bytes[5 + len] != expected {
        return Err(DecodeError::Checksum {
            expected,
            found: bytes[5 + len],
        });
    }
    if bytes[6 + len] != TERMINATOR {
        return Err(DecodeError::Terminator(bytes[6 + len]));
    }
    if let Some(i) = (7 + len..LEN).find(|&i| bytes[i] != 0) {
        return Err(DecodeError::Padding(i));
    }
    let payload = &bytes[5..5 + len];
    if payload == STARTUP {
        return Ok(Message::Startup);
    }
    if len != FRAME_PAYLOAD || payload[0] != FRAME_TAG {
        return Err(DecodeError::UnknownPayload);
    }
    let mut frame = Frame::blank();
    for (b, &value) in payload.iter().enumerate().skip(1) {
        if value & UNUSED_BIT != 0 {
            return Err(DecodeError::UnusedBit(b));
        }
        for (k, &bit) in PAIR_BITS.iter().enumerate() {
            if value & bit != 0 {
                // Pixel bytes 1–14 hold the odd row of each pair, columns 1 → 14; pixel bytes
                // 15–28 the even row, columns 14 → 1.
                let (row, col) = if b <= Frame::SIZE {
                    (2 * k + 1, b)
                } else {
                    (2 * k + 2, 29 - b)
                };
                frame.set(row, col, true);
            }
        }
    }
    Ok(Message::Frame(frame))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The checksum of a blank frame: (68 + 01 + 05 + 1d + 01) mod 256.
    const BLANK_CHECKSUM: u8 = 0x68 + 0x01 + 0x05 + 0x1d + 0x01;

    #[test]
    fn the_startup_report_is_exactly_the_spec_bytes() {
        let mut want = [0; LEN];
        want[..9].copy_from_slice(&[0x10, 0x68, 0x01, 0x05, 0x02, 0x03, 0x08, 0x7b, 0x16]);
        assert_eq!(startup_report().as_bytes(), &want);
    }

    #[test]
    fn a_blank_frame_is_the_tag_and_28_zeros() {
        let mut want = [0; LEN];
        want[..6].copy_from_slice(&[0x10, 0x68, 0x01, 0x05, 0x1d, 0x01]);
        want[34] = BLANK_CHECKSUM;
        want[35] = 0x16;
        assert_eq!(encode_frame(&Frame::blank()).as_bytes(), &want);
    }

    /// One lit pixel gives exactly one set bit, at the spec's position: pixel byte b is report
    /// byte 5 + b.
    #[test]
    fn single_pixels_land_on_the_spec_bit() {
        let cases = [
            ((1, 1), 1, 0x10),
            ((2, 1), 28, 0x10),
            ((13, 14), 14, 0x04),
            ((14, 14), 15, 0x04),
            ((7, 5), 5, 0x80),
            ((10, 3), 26, 0x01),
        ];
        for ((row, col), pixel_byte, bit) in cases {
            let mut frame = Frame::blank();
            frame.set(row, col, true);
            let mut want = [0; LEN];
            want[..6].copy_from_slice(&[0x10, 0x68, 0x01, 0x05, 0x1d, 0x01]);
            want[5 + pixel_byte] = bit;
            want[34] = BLANK_CHECKSUM.wrapping_add(bit);
            want[35] = 0x16;
            assert_eq!(
                encode_frame(&frame).as_bytes(),
                &want,
                "pixel ({row}, {col})"
            );
        }
    }

    fn any_frame() -> impl Strategy<Value = Frame> {
        proptest::collection::vec(any::<bool>(), Frame::SIZE * Frame::SIZE).prop_map(|on| {
            let mut frame = Frame::blank();
            for (i, &lit) in on.iter().enumerate() {
                frame.set(i / Frame::SIZE + 1, i % Frame::SIZE + 1, lit);
            }
            frame
        })
    }

    proptest! {
        #[test]
        fn a_frame_decodes_back_to_itself_without_bit_08(frame in any_frame()) {
            let report = encode_frame(&frame);
            prop_assert_eq!(decode(report.as_bytes()), Ok(Message::Frame(frame)));
            let pixel_bytes = &report.as_bytes()[6..34];
            prop_assert!(pixel_bytes.iter().all(|b| b & UNUSED_BIT == 0));
        }
    }

    #[test]
    fn the_startup_report_decodes_as_startup() {
        assert_eq!(decode(startup_report().as_bytes()), Ok(Message::Startup));
    }

    /// A valid frame with one pixel lit in every region, to mutate.
    fn frame_bytes() -> [u8; LEN] {
        let mut frame = Frame::blank();
        frame.set(3, 3, true);
        *encode_frame(&frame).as_bytes()
    }

    fn fix_checksum(bytes: &mut [u8; LEN]) {
        let n = usize::from(bytes[4]);
        bytes[5 + n] = checksum(&bytes[1..5 + n]);
    }

    #[test]
    fn decode_refuses_a_mutated_byte_in_every_region() {
        let mut b = frame_bytes();
        b[0] = 0x11;
        assert_eq!(decode(&b), Err(DecodeError::ReportId(0x11)));

        let mut b = frame_bytes();
        b[2] = 0x02;
        assert_eq!(decode(&b), Err(DecodeError::Header([0x68, 0x02, 0x05])));

        let mut b = frame_bytes();
        b[4] = 0x1e;
        assert_eq!(decode(&b), Err(DecodeError::Length(0x1e)));

        let mut b = frame_bytes();
        let expected = b[34];
        b[34] = expected.wrapping_add(1);
        assert_eq!(
            decode(&b),
            Err(DecodeError::Checksum {
                expected,
                found: expected.wrapping_add(1)
            })
        );

        let mut b = frame_bytes();
        b[35] = 0x17;
        assert_eq!(decode(&b), Err(DecodeError::Terminator(0x17)));

        let mut b = frame_bytes();
        b[40] = 0x01;
        assert_eq!(decode(&b), Err(DecodeError::Padding(40)));

        let mut b = frame_bytes();
        b[5 + 3] |= UNUSED_BIT;
        fix_checksum(&mut b);
        assert_eq!(decode(&b), Err(DecodeError::UnusedBit(3)));

        let mut b = frame_bytes();
        b[5] = 0x02;
        fix_checksum(&mut b);
        assert_eq!(decode(&b), Err(DecodeError::UnknownPayload));
    }

    #[test]
    fn decode_refuses_an_unknown_two_byte_payload() {
        let mut b = *startup_report().as_bytes();
        b[6] = 0x09;
        fix_checksum(&mut b);
        assert_eq!(decode(&b), Err(DecodeError::UnknownPayload));
    }
}
