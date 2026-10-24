//! Compound text from an X input method, decoded for zed-xim.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::RangeInclusive;
use encoding_rs::Encoding;

/// What failed to decode, with its bytes.
#[derive(Debug)]
pub enum DecodeError {
    InvalidEncoding(Vec<u8>),
    UnsupportedEncoding(Vec<u8>),
}

/// A set G0 or G1 holds.
#[derive(Clone, Copy)]
enum Set {
    Ascii,
    Roman,
    Katakana,
    Upper(&'static Encoding),
    Double(&'static Encoding, &'static [u8]),
}

impl Set {
    /// Decodes the leading run of `range` bytes and returns the rest.
    fn decode<'b>(
        self,
        bytes: &'b [u8],
        range: RangeInclusive<u8>,
        out: &mut String,
    ) -> Result<&'b [u8], DecodeError> {
        let end = bytes
            .iter()
            .position(|b| !range.contains(b))
            .unwrap_or(bytes.len());
        let (run, rest) = bytes.split_at(end);
        let invalid = || DecodeError::InvalidEncoding(run.to_vec());
        match self {
            Set::Ascii => out.extend(run.iter().map(|&b| char::from(b))),
            Set::Roman => out.extend(run.iter().map(|&b| match b {
                b'\\' => '¥',
                b'~' => '‾',
                _ => char::from(b),
            })),
            Set::Katakana => {
                for &b in run {
                    match b {
                        0xa1..=0xdf => out.extend(char::from_u32(u32::from(b) + 0xfec0)),
                        _ => return Err(invalid()),
                    }
                }
            }
            Set::Upper(encoding) => push(encoding, run, out).ok_or_else(invalid)?,
            Set::Double(encoding, prefix) => {
                let pairs = run.chunks_exact(2);
                if !pairs.remainder().is_empty() {
                    return Err(invalid());
                }
                let euc: Vec<u8> = pairs
                    .flat_map(|pair| prefix.iter().copied().chain(pair.iter().map(|b| b | 0x80)))
                    .collect();
                push(encoding, &euc, out).ok_or_else(invalid)?;
            }
        }
        Ok(rest)
    }
}

/// G0 and G1 hold across escapes and segments, as libX11 writes them.
pub fn compound_text_to_utf8(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut out = String::with_capacity(bytes.len());
    let mut g0 = Set::Ascii;
    let mut g1 = Set::Upper(encoding_rs::WINDOWS_1252);
    let mut rest = bytes;
    while let [byte, tail @ ..] = rest {
        rest = match byte {
            0x1b => escape(tail, &mut g0, &mut g1, &mut out)?,
            0x21..=0x7e => g0.decode(rest, 0x21..=0x7e, &mut out)?,
            0xa0..=0xff => g1.decode(rest, 0xa0..=0xff, &mut out)?,
            _ => {
                out.push(char::from(*byte));
                tail
            }
        };
    }
    Ok(out)
}

/// Reads the sequence after an ESC, and returns what follows it.
fn escape<'b>(
    bytes: &'b [u8],
    g0: &mut Set,
    g1: &mut Set,
    out: &mut String,
) -> Result<&'b [u8], DecodeError> {
    let end = bytes
        .iter()
        .position(|b| !(0x20..=0x2f).contains(b))
        .map_or(bytes.len(), |end| end + 1);
    let (sequence, rest) = bytes.split_at(end);
    let escaped = || [&[0x1b][..], sequence].concat();
    if !sequence.last().is_some_and(|b| (0x30..=0x7e).contains(b)) {
        return Err(DecodeError::InvalidEncoding(escaped()));
    }
    let unsupported = || DecodeError::UnsupportedEncoding(escaped());
    match sequence {
        b"(B" => *g0 = Set::Ascii,
        b"(J" => *g0 = Set::Roman,
        b")I" => *g1 = Set::Katakana,
        b"%@" => {}
        b"%G" => return utf8(rest, out),
        b"%/2" => return segment(rest, out),
        [b'-', last] => *g1 = Set::Upper(upper(*last).ok_or_else(unsupported)?),
        [b'$', b'(', last] => *g0 = double(*last).ok_or_else(unsupported)?,
        _ => return Err(unsupported()),
    }
    Ok(rest)
}

/// Reads UTF-8 up to `ESC % @`, or to the end, where ibus-x11's fallback leaves it open.
fn utf8<'b>(bytes: &'b [u8], out: &mut String) -> Result<&'b [u8], DecodeError> {
    let end = bytes
        .windows(3)
        .position(|w| w == b"\x1b%@")
        .unwrap_or(bytes.len());
    let text = &bytes[..end];
    let text =
        core::str::from_utf8(text).map_err(|_| DecodeError::InvalidEncoding(text.to_vec()))?;
    out.push_str(text);
    Ok(bytes.get(end + 3..).unwrap_or_default())
}

/// Reads an extended segment: its length, its encoding's name, STX, then its text.
fn segment<'b>(bytes: &'b [u8], out: &mut String) -> Result<&'b [u8], DecodeError> {
    let invalid = || DecodeError::InvalidEncoding(bytes.to_vec());
    let [m @ 0x80..=0xff, l @ 0x80..=0xff, rest @ ..] = bytes else {
        return Err(invalid());
    };
    let length = usize::from(m - 0x80) * 128 + usize::from(l - 0x80);
    let (segment, rest) = rest.split_at_checked(length).ok_or_else(invalid)?;
    let stx = segment
        .iter()
        .position(|&b| b == 0x02)
        .ok_or_else(invalid)?;
    let (name, text) = (&segment[..stx], &segment[stx + 1..]);
    let encoding = match name {
        b"big5-0" | b"big5hkscs-0" => encoding_rs::BIG5,
        b"gbk-0" => encoding_rs::GBK,
        _ => return Err(DecodeError::UnsupportedEncoding(name.to_vec())),
    };
    push(encoding, text, out).ok_or_else(|| DecodeError::InvalidEncoding(text.to_vec()))?;
    Ok(rest)
}

/// A 96-set's upper half, by final byte; windows-1252, 1254 and 874 match ISO 8859 there.
fn upper(last: u8) -> Option<&'static Encoding> {
    Some(match last {
        b'A' => encoding_rs::WINDOWS_1252,
        b'B' => encoding_rs::ISO_8859_2,
        b'C' => encoding_rs::ISO_8859_3,
        b'D' => encoding_rs::ISO_8859_4,
        b'F' => encoding_rs::ISO_8859_7,
        b'G' => encoding_rs::ISO_8859_6,
        b'H' => encoding_rs::ISO_8859_8,
        b'L' => encoding_rs::ISO_8859_5,
        b'M' => encoding_rs::WINDOWS_1254,
        b'T' => encoding_rs::WINDOWS_874,
        b'V' => encoding_rs::ISO_8859_10,
        b'Y' => encoding_rs::ISO_8859_13,
        b'_' => encoding_rs::ISO_8859_14,
        b'b' => encoding_rs::ISO_8859_15,
        b'f' => encoding_rs::ISO_8859_16,
        _ => return None,
    })
}

/// A 94 by 94 set, by its final byte, read as the EUC that holds it.
fn double(last: u8) -> Option<Set> {
    Some(match last {
        b'A' => Set::Double(encoding_rs::GBK, &[]),
        b'B' => Set::Double(encoding_rs::EUC_JP, &[]),
        b'C' => Set::Double(encoding_rs::EUC_KR, &[]),
        b'D' => Set::Double(encoding_rs::EUC_JP, &[0x8f]),
        _ => return None,
    })
}

fn push(encoding: &'static Encoding, bytes: &[u8], out: &mut String) -> Option<()> {
    out.push_str(&encoding.decode_without_bom_handling_and_without_replacement(bytes)?);
    Some(())
}
