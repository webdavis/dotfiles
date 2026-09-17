//! One image format, written by hand: a 1-bit greyscale PNG.
//!
//! NO IMAGE CRATE, AND NO COMPRESSOR EITHER. The only picture pns ever draws
//! is black text on white, which is one bit a pixel and about sixty
//! kilobytes against an upload ceiling of ten megabytes, so deflate's STORED
//! block type carries it uncompressed and legally. A dependency here would
//! be a compiler, a licence and a supply chain for a picture of some text.
//!
//! ONE BIT PER PIXEL, SET MEANING WHITE, which is what greyscale depth 1
//! means: a cleared bit is black ink.

/// The complete PNG file for a bitmap already packed one bit per pixel, its
/// scanlines top to bottom and each padded to a whole byte.
pub(super) fn greyscale_1bit(width: usize, scanlines: &[Vec<u8>]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(scanlines.len() * (scanlines[0].len() + 1));
    for scanline in scanlines {
        // FILTER 0, `None`: every scanline says it is stored as itself. A
        // predictor would only pay off with a compressor behind it.
        raw.push(0);
        raw.extend_from_slice(scanline);
    }
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(scanlines.len() as u32).to_be_bytes());
    // bit depth, colour type 0 (greyscale), deflate, filter method 0, no interlace
    header.extend_from_slice(&[1, 0, 0, 0, 0]);

    let mut png = SIGNATURE.to_vec();
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

/// The eight bytes that identify a PNG.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// One length-prefixed, CRC-suffixed chunk, appended.
fn chunk(into: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    into.extend_from_slice(&(body.len() as u32).to_be_bytes());
    into.extend_from_slice(kind);
    into.extend_from_slice(body);
    let mut crc = Crc::new();
    crc.eat(kind);
    crc.eat(body);
    into.extend_from_slice(&crc.finish().to_be_bytes());
}

/// A zlib stream whose deflate payload is stored rather than compressed.
///
/// BLOCKS OF 65535 BYTES, the largest a stored block's 16-bit length can
/// carry, and the last one carries the final flag.
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    // Deflate, 32 KiB window, no dictionary, and the check bits that make
    // the two-byte header a multiple of 31.
    let mut stream = vec![0x78, 0x01];
    let mut blocks = raw.chunks(STORED_BLOCK).peekable();
    if raw.is_empty() {
        stream.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        stream.push(u8::from(blocks.peek().is_none()));
        let length = block.len() as u16;
        stream.extend_from_slice(&length.to_le_bytes());
        stream.extend_from_slice(&(!length).to_le_bytes());
        stream.extend_from_slice(block);
    }
    stream.extend_from_slice(&adler32(raw).to_be_bytes());
    stream
}

/// The most a single stored deflate block can hold.
const STORED_BLOCK: usize = 65535;

/// The CRC-32 a PNG chunk ends with, computed a bit at a time.
///
/// NO LOOKUP TABLE. A table is eight times faster over a payload measured in
/// kilobytes, which nobody is waiting on, and 256 more words in this file.
struct Crc(u32);

impl Crc {
    fn new() -> Self {
        Crc(0xffff_ffff)
    }

    fn eat(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u32::from(*byte);
            for _ in 0..8 {
                let carry = self.0 & 1;
                self.0 >>= 1;
                if carry == 1 {
                    self.0 ^= 0xedb8_8320;
                }
            }
        }
    }

    fn finish(self) -> u32 {
        self.0 ^ 0xffff_ffff
    }
}

/// The checksum a zlib stream ends with.
fn adler32(bytes: &[u8]) -> u32 {
    let (mut low, mut high) = (1u32, 0u32);
    for byte in bytes {
        low = (low + u32::from(*byte)) % 65521;
        high = (high + low) % 65521;
    }
    (high << 16) | low
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksums_match_the_values_their_specifications_publish() {
        // THE ONE THING A FIXTURE CANNOT CATCH LATER: a checksum that is
        // wrong in a consistent way still produces a file, and every reader
        // refuses it. Both values are the published check words for the
        // ASCII string below.
        let mut crc = Crc::new();
        crc.eat(b"123456789");
        assert_eq!(crc.finish(), 0xcbf4_3926);
        assert_eq!(adler32(b"123456789"), 0x091e_01de);
    }

    #[test]
    fn the_file_carries_the_signature_the_header_and_the_three_chunks_in_order() {
        let png = greyscale_1bit(16, &vec![vec![0xff, 0x00]; 4]);
        assert_eq!(&png[..8], &SIGNATURE);
        assert_eq!(&png[12..16], b"IHDR");
        // Width, height, then depth 1 and greyscale.
        assert_eq!(&png[16..20], &16u32.to_be_bytes());
        assert_eq!(&png[20..24], &4u32.to_be_bytes());
        assert_eq!(&png[24..29], &[1, 0, 0, 0, 0]);
        let idat = png
            .windows(4)
            .position(|window| window == b"IDAT")
            .expect("the image data chunk");
        assert_eq!(&png[idat + 4..idat + 6], &[0x78, 0x01], "the zlib header");
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
    }

    #[test]
    fn a_payload_past_one_stored_block_is_split_and_only_the_last_block_is_final() {
        // THE MUTANT THIS PINS: the final flag set on every block, which
        // ends the stream at the first one and truncates a tall card.
        let scanlines = vec![vec![0xffu8; 999]; 100];
        let png = greyscale_1bit(7992, &scanlines);
        let idat = png
            .windows(4)
            .position(|window| window == b"IDAT")
            .expect("the image data chunk");
        let raw = 100 * 1000;
        assert!(raw > STORED_BLOCK, "the fixture must span two blocks");
        assert_eq!(png[idat + 6], 0, "the first block claimed to be the last");
        // Two blocks of 5 header bytes each, plus the zlib header and check.
        let payload = u32::from_be_bytes(png[idat - 4..idat].try_into().unwrap()) as usize;
        assert_eq!(payload, 2 + raw + 2 * 5 + 4);
    }
}
