//! NetEase Cloud Music protocol cryptography: AES-128-ECB PKCS7 and MD5.

use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use md5::{Digest, Md5};

/// Protocol key for NetEase PC EAPI encryption.
pub const EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

/// Encrypts `data` using AES-128-ECB with PKCS7 padding.
pub fn aes_128_ecb_encrypt(key: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let pad_len = 16 - (data.len() % 16);
    let mut padded = Vec::with_capacity(data.len() + pad_len);
    padded.extend_from_slice(data);
    padded.resize(data.len() + pad_len, pad_len as u8);

    let mut out = Vec::with_capacity(padded.len());
    for chunk in padded.chunks_exact(16) {
        let mut block = *GenericArray::from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    out
}

/// Computes the lowercase hex MD5 digest of `data`.
pub fn md5_hex(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(32);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02x}", byte);
    }
    hex
}

/// Encodes `bytes` as uppercase hexadecimal string.
pub fn hex_upper(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{:02X}", byte);
    }
    hex
}

/// Encrypts an EAPI request payload for `path` and JSON `text`.
pub fn eapi_encrypt(path: &str, text: &str) -> String {
    let digest_input = format!("nobody{}use{}md5forencrypt", path, text);
    let digest = md5_hex(digest_input.as_bytes());
    let plaintext = format!("{}-36cd479b6b5-{}-36cd479b6b5-{}", path, text, digest);
    let encrypted = aes_128_ecb_encrypt(EAPI_KEY, plaintext.as_bytes());
    hex_upper(&encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_hex() {
        assert_eq!(md5_hex(b"hello world"), "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_aes_128_ecb_encrypt() {
        let key = b"1234567890123456";
        let data = b"test plaintext";
        let enc = aes_128_ecb_encrypt(key, data);
        assert_eq!(enc.len() % 16, 0);
        assert!(!enc.is_empty());
    }

    #[test]
    fn test_eapi_encrypt() {
        let path = "/api/login/qrcode/unikey";
        let text = r#"{"type":3}"#;
        let result = eapi_encrypt(path, text);
        assert!(!result.is_empty());
        assert_eq!(result.len() % 32, 0);
    }
}
