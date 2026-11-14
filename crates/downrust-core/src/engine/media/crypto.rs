// SPDX-License-Identifier: GPL-3.0-or-later
//! AES-128 segment decryption for HLS (`METHOD=AES-128`): CBC mode with PKCS#7 padding.
use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};

use crate::error::{Error, Result};

type Aes128CbcDecryptor = cbc::Decryptor<Aes128>;

/// The IV used when a key tag has none: the media sequence number as a 16-byte big-endian value.
pub(crate) fn sequence_iv(sequence: u64) -> [u8; 16] {
    let mut iv = [0_u8; 16];
    iv[8..].copy_from_slice(&sequence.to_be_bytes());
    iv
}

/// Decrypts one segment. A wrong key or damaged data shows up as bad padding.
pub(crate) fn decrypt_segment(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>> {
    Aes128CbcDecryptor::new(key.into(), iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| Error::Transfer("segment decryption failed (bad padding)".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockEncryptMut;

    type Aes128CbcEncryptor = cbc::Encryptor<Aes128>;

    #[test]
    fn sequence_iv_is_big_endian_in_the_last_eight_bytes() {
        let iv = sequence_iv(0x0102);
        assert_eq!(&iv[..8], &[0; 8]);
        assert_eq!(&iv[8..], &[0, 0, 0, 0, 0, 0, 0x01, 0x02]);
    }

    #[test]
    fn decrypts_what_was_encrypted() {
        let key = [9_u8; 16];
        let iv = sequence_iv(4);
        let plain = vec![0xAB_u8; 1000];
        let cipher = Aes128CbcEncryptor::new((&key).into(), (&iv).into())
            .encrypt_padded_vec_mut::<Pkcs7>(&plain);
        assert_ne!(cipher, plain);
        assert_eq!(
            decrypt_segment(&key, &iv, &cipher).expect("decrypts"),
            plain
        );
    }

    #[test]
    fn wrong_key_is_reported() {
        let iv = sequence_iv(1);
        let cipher = Aes128CbcEncryptor::new((&[1_u8; 16]).into(), (&iv).into())
            .encrypt_padded_vec_mut::<Pkcs7>(b"segment");
        assert!(decrypt_segment(&[2_u8; 16], &iv, &cipher).is_err());
    }
}
