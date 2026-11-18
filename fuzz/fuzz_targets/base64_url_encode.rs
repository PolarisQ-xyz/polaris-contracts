#![no_main]

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // `base64_url::encode` indexes `dst` without bounds checks against
    // `src.len()`. Exercise it with exactly matching src/dst sizes so any
    // out-of-bounds indexing or panic surfaces here.
    let src_len = data.len();
    let dst_len = base64_url::encoded_len(src_len);
    let mut dst = vec![0u8; dst_len];

    let written = base64_url::encode(&mut dst, data);

    // The contract's encoder must not write past the provided buffer.
    assert!(written <= dst_len);

    // Compare against the reference URL-safe (no padding) encoding.
    let reference = URL_SAFE_NO_PAD.encode(data);
    assert_eq!(&dst[..written], reference.as_bytes());
});
