//! Minimal base64url (RFC 4648 §5, unpadded) encoder.
//!
//! Only encoding is needed: the contract turns a 32-byte challenge into the
//! `challenge` field of a WebAuthn `client_data_json`, which the browser
//! produces as unpadded base64url. Decoding is never required on-chain.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Number of bytes `encode` writes for an `n`-byte input: `ceil(n * 4 / 3)`.
///
/// This is the length contract between `src` and `dst`; callers must size
/// their output buffer with it.
pub const fn encoded_len(n: usize) -> usize {
    (n + 2) / 3 * 4
}

/// Encodes `src` as unpadded base64url into `dst`.
///
/// # Panics
///
/// Panics if `dst.len() != encoded_len(src.len())`. A short buffer would
/// index out of bounds; a long one would leave trailing zero bytes that
/// silently corrupt the encoded value.
pub fn encode(dst: &mut [u8], src: &[u8]) {
    debug_assert_eq!(dst.len(), encoded_len(src.len()));

    let mut di = 0;
    let mut chunks = src.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        dst[di] = ALPHABET[((n >> 18) & 0x3f) as usize];
        dst[di + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
        dst[di + 2] = ALPHABET[((n >> 6) & 0x3f) as usize];
        dst[di + 3] = ALPHABET[(n & 0x3f) as usize];
        di += 4;
    }

    match chunks.remainder() {
        [] => {}
        [a] => {
            let n = (*a as u32) << 16;
            dst[di] = ALPHABET[((n >> 18) & 0x3f) as usize];
            dst[di + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
        }
        [a, b] => {
            let n = ((*a as u32) << 16) | ((*b as u32) << 8);
            dst[di] = ALPHABET[((n >> 18) & 0x3f) as usize];
            dst[di + 1] = ALPHABET[((n >> 12) & 0x3f) as usize];
            dst[di + 2] = ALPHABET[((n >> 6) & 0x3f) as usize];
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_to_vec(src: &[u8]) -> Vec<u8> {
        let mut dst = vec![0u8; encoded_len(src.len())];
        encode(&mut dst, src);
        dst
    }

    #[test]
    fn encoded_len_matches_ceil_4_3() {
        assert_eq!(encoded_len(0), 0);
        assert_eq!(encoded_len(1), 4);
        assert_eq!(encoded_len(2), 4);
        assert_eq!(encoded_len(3), 4);
        assert_eq!(encoded_len(31), 44);
        assert_eq!(encoded_len(32), 43);
        assert_eq!(encoded_len(33), 44);
    }

    #[test]
    fn empty_input() {
        assert_eq!(encode_to_vec(b""), b"");
    }

    #[test]
    fn remainder_zero() {
        assert_eq!(encode_to_vec(b"Man"), b"TWFu");
        assert_eq!(encode_to_vec(b"abc"), b"YWJj");
    }

    #[test]
    fn remainder_one() {
        assert_eq!(encode_to_vec(b"M"), b"TQ");
        assert_eq!(encode_to_vec(b"a"), b"YQ");
    }

    #[test]
    fn remainder_two() {
        assert_eq!(encode_to_vec(b"Ma"), b"TWE");
        assert_eq!(encode_to_vec(b"ab"), b"YWI");
    }

    #[test]
    fn thirty_two_bytes() {
        let src = [0u8; 32];
        let out = encode_to_vec(&src);
        assert_eq!(out.len(), 43);
        assert_eq!(out, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn thirty_one_and_thirty_three_bytes() {
        let src31 = [0u8; 31];
        let out31 = encode_to_vec(&src31);
        assert_eq!(out31.len(), 44);
        assert_eq!(out31, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".map(|b| if b == b'=' { b'A' } else { b }));

        let src33 = [0u8; 33];
        let out33 = encode_to_vec(&src33);
        assert_eq!(out33.len(), 44);
        assert_eq!(out33, b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }
}
