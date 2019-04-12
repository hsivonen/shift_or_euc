// Copyright 2018 Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use encoding_rs::*;

/// Returns the index of the first non-ASCII byte or the first
/// 0x1B, whichever comes first, or the length of the buffer
/// if neither is found.
fn find_non_ascii_or_escape(buffer: &[u8]) -> usize {
    let ascii_up_to = Encoding::ascii_valid_up_to(buffer);
    if let Some(escape) = memchr::memchr(0x1B, &buffer[..ascii_up_to]) {
        escape
    } else {
        ascii_up_to
    }
}

fn find_euc_jp_half_width(buffer: &[u8]) -> usize {
    if let Some(half_width) = memchr::memchr(0x8E, buffer) {
        half_width
    } else {
        buffer.len()
    }
}

fn find_shift_jis_half_width(buffer: &[u8]) -> usize {
    for (i, &b) in buffer.into_iter().enumerate() {
        match b {
            0xA1...0xDF => {
                return i;
            }
            _ => {}
        }
    }
    buffer.len()
}

pub struct Detector {
    shift_jis_decoder: Decoder,
    euc_jp_decoder: Decoder,
    second_byte_in_escape: u8,
    iso_2022_jp_disqualified: bool,
    escape_seen: bool,
    finished: bool,
}

impl Detector {
    pub fn new(allow_2022: bool) -> Self {
        Detector {
            shift_jis_decoder: SHIFT_JIS.new_decoder_without_bom_handling(),
            euc_jp_decoder: EUC_JP.new_decoder_without_bom_handling(),
            second_byte_in_escape: 0,
            iso_2022_jp_disqualified: !allow_2022,
            escape_seen: false,
            finished: false,
        }
    }

    pub fn feed(&mut self, buffer: &[u8], last: bool) -> Option<&'static Encoding> {
        assert!(
            !self.finished,
            "Tried to used a detector that has finished."
        );
        self.finished = true; // Will change back to false unless we return early
        let mut i = 0;
        if !self.iso_2022_jp_disqualified {
            if !self.escape_seen {
                i = find_non_ascii_or_escape(buffer);
            }
            while i < buffer.len() {
                let byte = buffer[i];
                if byte > 0x7F {
                    self.iso_2022_jp_disqualified = true;
                    break;
                }
                if !self.escape_seen && byte == 0x1B {
                    self.escape_seen = true;
                    i += 1;
                    continue;
                }
                if self.escape_seen && self.second_byte_in_escape == 0 {
                    self.second_byte_in_escape = byte;
                    i += 1;
                    continue;
                }
                match (self.second_byte_in_escape, byte) {
                    (0x28, 0x42) | (0x28, 0x4A) | (0x28, 0x49) | (0x24, 0x40) | (0x24, 0x42) => {
                        return Some(ISO_2022_JP);
                    }
                    _ => {}
                }
                if self.escape_seen {
                    self.iso_2022_jp_disqualified = true;
                    break;
                }
                i += 1;
            }
        }
        let mut output = [0u16; 1024];
        let mut euc_jp_had_error = false;
        let mut euc_jp_total_read = i;
        let euc_jp_non_half_width_up_to = i + find_euc_jp_half_width(&buffer[i..]);
        loop {
            let (result, read, _written) = self.euc_jp_decoder.decode_to_utf16_without_replacement(
                &buffer[euc_jp_total_read..euc_jp_non_half_width_up_to],
                &mut output[..],
                last,
            );
            euc_jp_total_read += read;
            if let DecoderResult::Malformed(_, _) = result {
                euc_jp_had_error = true;
                break;
            }
            if result == DecoderResult::InputEmpty {
                break;
            }
        }
        let mut shift_jis_total_read = i;
        let mut shift_jis_had_error = false;
        let shift_jis_non_half_width_up_to =
            i + find_shift_jis_half_width(&buffer[i..euc_jp_total_read]);
        loop {
            let (result, read, _written) =
                self.shift_jis_decoder.decode_to_utf16_without_replacement(
                    &buffer[shift_jis_total_read..shift_jis_non_half_width_up_to],
                    &mut output[..],
                    last,
                );
            shift_jis_total_read += read;
            if let DecoderResult::Malformed(_, _) = result {
                shift_jis_had_error = true;
                break;
            }
            if result == DecoderResult::InputEmpty {
                break;
            }
        }
        if euc_jp_total_read < shift_jis_total_read {
            return Some(SHIFT_JIS);
        }
        if shift_jis_total_read < euc_jp_total_read {
            return Some(EUC_JP);
        }
        assert_eq!(euc_jp_total_read, shift_jis_total_read);
        // If equal, error wins over half-width katakana
        if euc_jp_had_error {
            return Some(SHIFT_JIS);
        }
        if shift_jis_had_error {
            return Some(EUC_JP);
        }
        // In case of a tie, Shift_JIS wins
        if shift_jis_total_read < buffer.len() {
            return Some(SHIFT_JIS);
        }
        if last {
            return Some(SHIFT_JIS);
        }
        self.finished = false;
        None
    }
}

// Any copyright to the test code below this comment is dedicated to the
// Public Domain. http://creativecommons.org/publicdomain/zero/1.0/

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_iso_2022_jp() {
        let mut detector = Detector::new(true);
        assert_eq!(
            detector.feed(b"abc\x1B\x28\x42\xFF", true),
            Some(ISO_2022_JP)
        );
    }

    #[test]
    fn test_error_precedence() {
        let mut detector = Detector::new(true);
        assert_eq!(detector.feed(b"abc\xFF", true), Some(SHIFT_JIS));
    }

    #[test]
    fn test_invalid_euc_jp() {
        let mut detector = Detector::new(true);
        assert_eq!(detector.feed(b"abc\x81\x40", true), Some(SHIFT_JIS));
    }

    #[test]
    fn test_invalid_shift_jis() {
        let mut detector = Detector::new(true);
        assert_eq!(detector.feed(b"abc\xEB\xA8", true), Some(EUC_JP));
    }

    #[test]
    fn test_invalid_shift_jis_before_invalid_euc_jp() {
        let mut detector = Detector::new(true);
        assert_eq!(detector.feed(b"abc\xEB\xA8\x81\x40", true), Some(EUC_JP));
    }

}
