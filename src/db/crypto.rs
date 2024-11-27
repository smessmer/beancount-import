use anyhow::{bail, Result};

// TODO Maybe we should factor out cryfs's crypto implementation into a separate crate and use that here.

pub trait Cipher {
    type EncryptionKey;

    fn new_key() -> Self::EncryptionKey;
    fn with_key(key: &Self::EncryptionKey) -> Self;
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}

mod xchacha20poly1305cipher {
    use chacha20poly1305::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Key, XChaCha20Poly1305,
    };

    use super::*;

    const NONCE_LEN: usize = 24;

    pub struct XChaCha20Poly1305Cipher {
        cipher: XChaCha20Poly1305,
    }

    impl Cipher for XChaCha20Poly1305Cipher {
        type EncryptionKey = Key;

        fn new_key() -> Key {
            XChaCha20Poly1305::generate_key(&mut OsRng)
        }

        fn with_key(key: &Key) -> Self {
            Self {
                cipher: XChaCha20Poly1305::new(key),
            }
        }

        fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
            let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
            assert_eq!(NONCE_LEN, nonce.len());
            let ciphertext = self.cipher.encrypt(&nonce, plaintext)?;

            let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
            result.extend_from_slice(&nonce);
            result.extend_from_slice(&ciphertext);

            Ok(result)
        }

        fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
            if ciphertext.len() < NONCE_LEN {
                bail!("Ciphertext too small for nonce");
            }
            let nonce = &ciphertext[..NONCE_LEN];
            let ciphertext = &ciphertext[NONCE_LEN..];

            let plaintext = self.cipher.decrypt(nonce.into(), ciphertext)?;
            Ok(plaintext)
        }
    }
}
pub use xchacha20poly1305cipher::XChaCha20Poly1305Cipher;

#[cfg(test)]
mod tests {
    use chacha20poly1305::Key;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    use super::*;

    const KEY_SIZE: usize = 32;

    fn key(seed: u64) -> Key {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut key_bytes = [0; KEY_SIZE];
        rng.fill_bytes(&mut key_bytes);
        Key::clone_from_slice(&key_bytes)
    }

    #[test]
    fn given_emptydata_when_encrypted_then_canbedecrypted() {
        let plaintext = &[];
        let cipher = XChaCha20Poly1305Cipher::with_key(&key(1));
        let ciphertext = cipher.encrypt(plaintext).unwrap();
        let decrypted_plaintext = cipher.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted_plaintext);
    }

    #[test]
    fn given_somedata_when_encrypted_then_canbedecrypted() {
        let plaintext = hex::decode("0ffc9a43e15ccfbef1b0880167df335677c9005948eeadb31f89b06b90a364ad03c6b0859652dca960f8fa60c75747c4f0a67f50f5b85b800468559ea1a816173c0abaf5df8f02978a54b250bc57c7c6a55d4d245014722c0b1764718a6d5ca654976370").unwrap();

        let cipher = XChaCha20Poly1305Cipher::with_key(&key(1));
        let ciphertext = cipher.encrypt(&plaintext).unwrap();
        let decrypted_plaintext = cipher.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted_plaintext);
    }

    #[test]
    fn given_invalidciphertext_then_doesntdecrypt() {
        let plaintext = hex::decode("0ffc9a43e15ccfbef1b0880167df335677c9005948eeadb31f89b06b90a364ad03c6b0859652dca960f8fa60c75747c4f0a67f50f5b85b800468559ea1a816173c0abaf5df8f02978a54b250bc57c7c6a55d4d245014722c0b1764718a6d5ca654976370").unwrap();

        let cipher = XChaCha20Poly1305Cipher::with_key(&key(1));
        let mut ciphertext = cipher.encrypt(&plaintext).unwrap();
        ciphertext[20] ^= 1;
        let decrypted_plaintext = cipher.decrypt(&ciphertext);
        assert!(decrypted_plaintext.is_err());
    }

    #[test]
    fn given_toosmallciphertext_then_doesntdecrypt() {
        let plaintext = hex::decode("0ffc9a43e15ccfbef1b0880167df335677c9005948eeadb31f89b06b90a364ad03c6b0859652dca960f8fa60c75747c4f0a67f50f5b85b800468559ea1a816173c0abaf5df8f02978a54b250bc57c7c6a55d4d245014722c0b1764718a6d5ca654976370").unwrap();

        let cipher = XChaCha20Poly1305Cipher::with_key(&key(1));
        let ciphertext = cipher.encrypt(&plaintext).unwrap();
        let ciphertext = &ciphertext[..(ciphertext.len() - 1)];
        let decrypted_plaintext = cipher.decrypt(&ciphertext);
        assert!(decrypted_plaintext.is_err());
    }

    #[test]
    fn given_emptyciphertext_then_doesntdecrypt() {
        let cipher = XChaCha20Poly1305Cipher::with_key(&key(1));
        let ciphertext = &[];
        let decrypted_plaintext = cipher.decrypt(ciphertext);
        assert!(decrypted_plaintext.is_err());
    }

    #[test]
    fn given_differentkey_then_doesntdecrypt() {
        let plaintext =hex::decode("0ffc9a43e15ccfbef1b0880167df335677c9005948eeadb31f89b06b90a364ad03c6b0859652dca960f8fa60c75747c4f0a67f50f5b85b800468559ea1a816173c0abaf5df8f02978a54b250bc57c7c6a55d4d245014722c0b1764718a6d5ca654976370").unwrap();

        let cipher1 = XChaCha20Poly1305Cipher::with_key(&key(1));
        let cipher2 = XChaCha20Poly1305Cipher::with_key(&key(2));
        let ciphertext = cipher1.encrypt(&plaintext).unwrap();
        let decrypted_plaintext = cipher2.decrypt(&ciphertext);
        assert!(decrypted_plaintext.is_err());
    }
}
