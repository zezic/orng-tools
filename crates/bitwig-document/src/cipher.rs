//! The Dag stream cipher guarding document sections.
//!
//! Symmetric XOR: encrypting and decrypting are the same operation, and a
//! same-length edit can be spliced into the plaintext and re-encrypted under
//! the original nonce without disturbing any other byte.

/// Section key, from Bitwig's own key table.
const SECTION_KEY: [u8; 128] = [0; 128];

pub const NONCE_LEN: usize = 16;

struct Dag {
    key_tail: &'static [u8],
    stream: Vec<u8>,
    key_idx: usize,
    stream_idx: usize,
    cycle: u32,
}

impl Dag {
    fn new(nonce: &[u8]) -> Self {
        let mut stream = Vec::with_capacity(nonce.len() + 16);
        stream.extend_from_slice(nonce);
        stream.extend_from_slice(&SECTION_KEY[..16]);
        Dag { key_tail: &SECTION_KEY[16..], stream, key_idx: 0, stream_idx: 0, cycle: 0 }
    }

    fn apply_byte(&mut self, input: u8) -> u8 {
        if self.key_idx >= self.key_tail.len() {
            self.key_idx = 0;
            self.cycle += 1;
        }
        if self.stream_idx >= self.stream.len() {
            self.stream_idx = 0;
        }
        let k = self.key_tail[self.key_idx];
        let s = self.stream[self.stream_idx] as u32;
        self.key_idx += 1;
        self.stream_idx += 1;
        let rot = self.cycle & 7;
        let rotated = (s >> rot | s << (8 - rot)) as u8;
        input ^ k ^ rotated
    }

    fn apply(&mut self, data: &[u8]) -> Vec<u8> {
        data.iter().map(|&b| self.apply_byte(b)).collect()
    }
}

/// Transform `data` under `nonce`. Encryption and decryption are identical.
pub fn transform(nonce: &[u8; NONCE_LEN], data: &[u8]) -> Vec<u8> {
    Dag::new(nonce).apply(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_is_an_involution() {
        let nonce = [7u8; NONCE_LEN];
        let plain = b"device_uuid and some padding to cross the key cycle boundary".repeat(4);
        assert_eq!(transform(&nonce, &transform(&nonce, &plain)), plain);
    }
}
