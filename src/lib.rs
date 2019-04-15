// Copyright 2018 Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use encoding_rs::Decoder;
use encoding_rs::DecoderResult;
use encoding_rs::Encoding;
use encoding_rs::EUC_JP;
use encoding_rs::ISO_2022_JP;
use encoding_rs::SHIFT_JIS;

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

#[inline(always)]
fn feed_decoder(decoder: &mut Decoder, byte: u8, last: bool) -> bool {
    let mut output = [0u16; 1];
    let input = [byte];
    let (result, _read, written) = decoder.decode_to_utf16_without_replacement(
        if last { b"" } else { &input },
        &mut output,
        last,
    );
    match result {
        DecoderResult::InputEmpty => {
            if written == 1 {
                match output[0] {
                    0xFF61...0xFF9F => {
                        return false;
                    }
                    _ => {}
                }
            }
        }
        DecoderResult::Malformed(_, _) => {
            return false;
        }
        DecoderResult::OutputFull => {
            unreachable!();
        }
    }
    true
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
        for &byte in &buffer[i..] {
            if !feed_decoder(&mut self.euc_jp_decoder, byte, false) {
                return Some(SHIFT_JIS);
            }
            if !feed_decoder(&mut self.shift_jis_decoder, byte, false) {
                return Some(EUC_JP);
            }
        }
        if last {
            if !feed_decoder(&mut self.euc_jp_decoder, 0, true) {
                return Some(SHIFT_JIS);
            }
            if !feed_decoder(&mut self.shift_jis_decoder, 0, true) {
                return Some(EUC_JP);
            }
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
