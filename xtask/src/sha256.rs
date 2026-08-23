//! SHA-256, written out here rather than pulled in.
//!
//! A content hash is the manifest's promise that the file on this disk is
//! the file the manifest was written against. That promise is worthless
//! unless the hash means the same thing on every machine and in every
//! release, which rules out `std::hash::DefaultHasher` — its algorithm is
//! explicitly allowed to change between Rust versions. SHA-256 is sixty
//! lines and a published test vector; a dependency is a graph.

/// The round constants: the first thirty-two bits of the fractional parts
/// of the cube roots of the first sixty-four primes (FIPS 180-4, §4.2.2).
#[allow(clippy::unreadable_literal)]
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The initial state: the fractional parts of the square roots of the
/// first eight primes (FIPS 180-4, §5.3.3).
#[allow(clippy::unreadable_literal)]
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// A running SHA-256. Fed in chunks so a 400 MB FBX never lands in memory.
pub struct Hasher {
    state: [u32; 8],
    block: [u8; 64],
    filled: usize,
    /// Message length in BITS, which is what the padding encodes.
    bits: u64,
}

impl Hasher {
    pub const fn new() -> Self {
        Self {
            state: H0,
            block: [0; 64],
            filled: 0,
            bits: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.bits = self.bits.wrapping_add(data.len() as u64 * 8);
        while !data.is_empty() {
            let take = (64 - self.filled).min(data.len());
            self.block[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled == 64 {
                let block = self.block;
                compress(&mut self.state, &block);
                self.filled = 0;
            }
        }
    }

    /// Pad, absorb the length, and hand back the digest as lowercase hex —
    /// which is the only form anything here ever wants it in.
    pub fn finish(mut self) -> String {
        let bits = self.bits;
        self.update_raw(&[0x80]);
        while self.filled != 56 {
            self.update_raw(&[0]);
        }
        self.update_raw(&bits.to_be_bytes());
        let mut hex = String::with_capacity(64);
        for word in self.state {
            for byte in word.to_be_bytes() {
                hex.push_str(HEX[(byte >> 4) as usize]);
                hex.push_str(HEX[(byte & 0xf) as usize]);
            }
        }
        hex
    }

    /// [`Hasher::update`] without counting the bytes, for the padding —
    /// which must not extend the length it is encoding.
    fn update_raw(&mut self, data: &[u8]) {
        let counted = self.bits;
        self.update(data);
        self.bits = counted;
    }
}

const HEX: [&str; 16] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f",
];

/// One 64-byte block, FIPS 180-4 §6.2.2. The eight single-letter working
/// variables are the standard's own names; renaming them would make this
/// harder to check against the document, not easier to read.
#[allow(clippy::many_single_char_names)]
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, word) in w.iter_mut().take(16).enumerate() {
        let b = &block[i * 4..i * 4 + 4];
        *word = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// The digest of a whole file, read 64 KiB at a time.
pub fn of_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            return Ok(hasher.finish());
        }
        hasher.update(&buf[..read]);
    }
}

/// The digest of a slice already in hand. Nothing at run time hashes
/// something it has not already streamed off a disk; this is here for
/// the published vectors below.
#[cfg(test)]
pub fn of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The hash this repo writes down is the hash everyone else means
    /// by SHA-256.** A hand-rolled digest that is merely self-consistent
    /// would content-address the cache perfectly and still disagree with
    /// `sha256sum`, which is what the owner will reach for when a
    /// manifest line and a downloaded pack argue.
    ///
    /// The three vectors are FIPS 180-4's own (`""` and `"abc"`) plus the
    /// two-block case, which is the one that catches a padding bug: a
    /// message of 56..64 bytes needs a whole extra block for its length.
    #[test]
    fn a_digest_here_is_the_digest_everyone_else_publishes() {
        assert_eq!(
            of_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            of_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            of_bytes(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// **A file's digest does not depend on how the reader chopped it
    /// up.** The streaming path reads 64 KiB at a time and a Synty FBX is
    /// bigger than that, so a block boundary lands mid-file on every real
    /// asset and never on a short test string.
    #[test]
    fn a_digest_does_not_depend_on_the_size_of_the_reads() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let whole = of_bytes(&data);
        for chunk in [1usize, 7, 63, 64, 65, 1000, 65_536] {
            let mut hasher = Hasher::new();
            for part in data.chunks(chunk) {
                hasher.update(part);
            }
            assert_eq!(hasher.finish(), whole, "chunked by {chunk}");
        }
    }
}
