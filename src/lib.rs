// Copyright 2018 Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use encoding_rs::*;

#[derive(Debug)]
struct Statistics {
    kanji: usize,
    hiragana: usize,
    katakana: usize,
    half_width: usize,
}

impl Statistics {
    fn new() -> Self {
        Statistics {
            kanji: 0,
            hiragana: 0,
            katakana: 0,
            half_width: 0,
        }
    }

    fn accumulate(&mut self, buffer: &[u16]) {
        for &unit in buffer.into_iter() {
            match unit {
                0x3040...0x309F => {
                    self.hiragana += 1;
                }
                0x4E00...0x9FEF => {
                    self.kanji += 1;
                }
                0x30A0...0x30FF => {
                    self.katakana += 1;
                }
                0xFF61...0xFF9F => {
                    self.half_width += 1;
                }
                _ => {}
            }
        }
    }

    fn deviation(&self) -> f32 {
        let total = self.hiragana + self.katakana + self.kanji + self.half_width;
        if total == 0 {
            // Avoid division by zero at the end
            return 0.0;
        }
        let total_float = total as f32;
        let expect_hiragana = total_float * 0.6;
        let expect_katakana = total_float * 0.1;
        let expect_kanji = total_float * 0.3;
        let hiragana_difference = self.hiragana as f32 - expect_hiragana;
        let katakana_difference = self.katakana as f32 - expect_katakana;
        let kanji_difference = self.kanji as f32 - expect_kanji;

        // The difference compounds, since it counts in both
        // the category that is under and the category that is
        // over the expectation
        (self.half_width as f32
            + hiragana_difference.abs()
            + kanji_difference.abs()
            + katakana_difference.abs())
            / total_float
    }
}

pub struct Detector {
    shift_jis_decoder: Decoder,
    euc_jp_decoder: Decoder,
    shift_jis_statistics: Statistics,
    euc_jp_statistics: Statistics,
    second_byte_in_escape: u8,
    iso_2022_jp_disqualified: bool,
    escape_seen: bool,
    finished: bool,
}

impl Detector {
    pub fn new() -> Self {
        Detector {
            shift_jis_decoder: SHIFT_JIS.new_decoder_without_bom_handling(),
            euc_jp_decoder: EUC_JP.new_decoder_without_bom_handling(),
            shift_jis_statistics: Statistics::new(),
            euc_jp_statistics: Statistics::new(),
            second_byte_in_escape: 0,
            iso_2022_jp_disqualified: false,
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
        if !self.iso_2022_jp_disqualified {
            for &byte in buffer.into_iter() {
                if byte > 0x7F {
                    self.iso_2022_jp_disqualified = true;
                    break;
                }
                if !self.escape_seen && byte == 0x1B {
                    self.escape_seen = true;
                    continue;
                }
                if self.escape_seen && self.second_byte_in_escape == 0 {
                    self.second_byte_in_escape = byte;
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
            }
        }
        // TODO: Skip bytes already examined
        let mut output = [0u16; 1024];
        let mut euc_jp_had_error = false;
        let mut euc_jp_total_read = 0;
        loop {
            let (result, read, written) = self.euc_jp_decoder.decode_to_utf16_without_replacement(
                &buffer[euc_jp_total_read..],
                &mut output[..],
                last,
            );
            euc_jp_total_read += read;
            if let DecoderResult::Malformed(_, _) = result {
                euc_jp_had_error = true;
                break;
            }
            self.euc_jp_statistics.accumulate(&output[..written]);
            if result == DecoderResult::InputEmpty {
                break;
            }
        }
        let mut shift_jis_total_read = 0;
        loop {
            let (result, read, written) =
                self.shift_jis_decoder.decode_to_utf16_without_replacement(
                    &buffer[shift_jis_total_read..],
                    &mut output[..],
                    last,
                );
            shift_jis_total_read += read;
            if let DecoderResult::Malformed(_, _) = result {
                if euc_jp_had_error && euc_jp_total_read <= shift_jis_total_read {
                    return Some(SHIFT_JIS);
                }
                return Some(EUC_JP);
            }
            self.shift_jis_statistics.accumulate(&output[..written]);
            if result == DecoderResult::InputEmpty {
                break;
            }
        }
        if last {
            if self.shift_jis_statistics.deviation() > self.euc_jp_statistics.deviation() {
                return Some(EUC_JP);
            }
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
        let mut detector = Detector::new();
        assert_eq!(
            detector.feed(b"abc\x1B\x28\x42\xFF", true),
            Some(ISO_2022_JP)
        );
    }

    #[test]
    fn test_error_precedence() {
        let mut detector = Detector::new();
        assert_eq!(detector.feed(b"abc\xFF", true), Some(SHIFT_JIS));
    }

    #[test]
    fn test_invalid_euc_jp() {
        let mut detector = Detector::new();
        assert_eq!(detector.feed(b"abc\x81\x40", true), Some(SHIFT_JIS));
    }

    #[test]
    fn test_invalid_shift_jis() {
        let mut detector = Detector::new();
        assert_eq!(detector.feed(b"abc\xEB\xA8", true), Some(EUC_JP));
    }

    #[test]
    fn test_invalid_shift_jis_before_invalid_euc_jp() {
        let mut detector = Detector::new();
        assert_eq!(detector.feed(b"abc\xEB\xA8\x81\x40", true), Some(EUC_JP));
    }

}
