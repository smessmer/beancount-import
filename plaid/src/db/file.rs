use anyhow::{anyhow, ensure, Result};
use crc::{Crc, CRC_32_BZIP2};
use std::path::PathBuf;

use super::{crypto::Cipher, database::DatabaseV1, Database, XChaCha20Poly1305Cipher};

pub struct DatabaseFile {
    database: DatabaseV1,
    db_path: PathBuf,
    db_cipher: XChaCha20Poly1305Cipher,
    modified: bool,
}

impl DatabaseFile {
    pub fn new(database: DatabaseV1, db_path: PathBuf, db_cipher: XChaCha20Poly1305Cipher) -> Self {
        Self {
            database,
            db_path,
            db_cipher,
            modified: false,
        }
    }

    pub fn database(&self) -> &DatabaseV1 {
        &self.database
    }

    pub fn database_mut(&mut self) -> &mut DatabaseV1 {
        self.modified = true;
        &mut self.database
    }

    /// Returns Ok(None) if the db file doesn't exist yet
    pub async fn load(
        db_path: PathBuf,
        db_cipher: XChaCha20Poly1305Cipher,
    ) -> Result<Option<Self>> {
        log::info!("Loading database...");
        if !tokio::fs::try_exists(&db_path).await? {
            return Ok(None);
        }

        let content_ciphertext = tokio::fs::read(&db_path).await?;
        let content_plaintext = db_cipher.decrypt(&content_ciphertext)?;
        let content_decompressed = zstd::bulk::decompress(
            &content_plaintext,
            content_plaintext.len().max(1024 * 1024 * 1024),
        )?;
        let crc = crc();
        let (parsed, remaining): (Database, &[u8]) =
            postcard::take_from_bytes_crc32(&content_decompressed, crc.digest())?;
        let Database::V1(database) = parsed;
        ensure!(0 == remaining.len(), "File had extra bytes");

        log::info!("Loading database...done");

        Ok(Some(Self {
            database,
            db_path,
            db_cipher,
            modified: false,
        }))
    }

    pub async fn save_if_modified(self) -> Result<()> {
        if self.modified {
            self.save().await
        } else {
            Ok(())
        }
    }

    async fn save(self) -> Result<()> {
        log::info!("Saving database...");

        let crc = crc();
        let content_plaintext =
            postcard::to_stdvec_crc32(&Database::V1(self.database), crc.digest())?;
        let content_compressed = zstd::bulk::compress(
            &content_plaintext,
            zstd::compression_level_range().last().unwrap(),
        )?;
        let content_ciphertext = self.db_cipher.encrypt(&content_compressed)?;

        // First write to temporary file so we don't lose data if writing fails halfway
        let filename = self
            .db_path
            .file_name()
            .ok_or_else(|| anyhow!("Path has no filename"))?
            .to_str()
            .ok_or_else(|| anyhow!("Filename isn't valid utf-8"))?;
        let tmppath = self.db_path.with_file_name(format!("{}.temp:", filename));
        tokio::fs::write(&tmppath, content_ciphertext).await?;

        // Ok, writing succeeded, let's now replace the real file with the tmpfile
        tokio::fs::rename(&tmppath, self.db_path).await?;

        log::info!("Saving database...done");

        Ok(())
    }
}

fn crc() -> Crc<u32> {
    // TODO Which crc algorithm should we use?
    Crc::<u32>::new(&CRC_32_BZIP2)
}

#[cfg(test)]
impl PartialEq for DatabaseFile {
    fn eq(&self, other: &Self) -> bool {
        self.database == other.database && self.db_path == other.db_path
    }
}
#[cfg(test)]
impl std::fmt::Debug for DatabaseFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseFile")
            .field("database", &self.database)
            .field("db_path", &self.db_path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use common_macros::hash_map;
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    use crate::db::{
        account::{Account, AccountType, BeancountAccountInfo, PlaidAccountInfo},
        bank_connection::BankConnection,
        crypto::{self, XChaCha20Poly1305Cipher},
        database::DatabaseV1,
        plaid_auth::DbPlaidAuth,
        AccessToken, AccountId,
    };

    use super::*;

    const KEY_SIZE: usize = 32;

    fn cipher(seed: u64) -> XChaCha20Poly1305Cipher {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut key_bytes = [0; KEY_SIZE];
        rng.fill_bytes(&mut key_bytes);

        XChaCha20Poly1305Cipher::with_key(
            <crypto::XChaCha20Poly1305Cipher as crypto::Cipher>::EncryptionKey::from_slice(
                &key_bytes,
            ),
        )
    }

    fn some_db_1() -> DatabaseV1 {
        DatabaseV1 {
            plaid_auth: DbPlaidAuth::new("client-id".to_string(), "secret".to_string()),
            bank_connections: vec![BankConnection::new(
                "connection-name-1".to_string(),
                AccessToken::new("access-token-1".to_string()),
                hash_map![
                    AccountId("account-1".to_string()) => Account::new_connected(PlaidAccountInfo {
                        name: "Account 1".to_string(),
                        official_name: None,
                        mask: None,
                        type_: "account-type".to_string(),
                        subtype: None,
                    }, BeancountAccountInfo{
                        ty: AccountType::Assets,
                        name_parts: vec!["Part1".to_string(), "Part2".to_string()],
                    }), AccountId("account-2".to_string()) =>
                    Account::new_connected(PlaidAccountInfo {
                        name: "Account 2".to_string(),
                        official_name: None,
                        mask: None,
                        type_: "account-type".to_string(),
                        subtype: None,
                    }, BeancountAccountInfo{
                        ty: AccountType::Liabilities,
                        name_parts: vec!["Part1".to_string(), "Part2".to_string()],
                    }),
                ],
            )],
        }
    }

    fn some_db_2() -> DatabaseV1 {
        DatabaseV1 {
            plaid_auth: DbPlaidAuth::new("client-id".to_string(), "secret".to_string()),
            bank_connections: vec![BankConnection::new(
                "connection-name-1".to_string(),
                AccessToken::new("access-token-2".to_string()),
                hash_map![AccountId("account-100".to_string()) => Account::new_connected(PlaidAccountInfo {
                    name: "Account 100".to_string(),
                    official_name: None,
                    mask: None,
                    type_: "account-type".to_string(),
                    subtype: None,
                }, BeancountAccountInfo{
                    ty: AccountType::Assets,
                    name_parts: vec!["Part1".to_string(), "Part2".to_string()],
                })],
            )],
        }
    }

    #[tokio::test]
    async fn load_nonexisting() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let loaded = DatabaseFile::load(tempfile, cipher(1)).await.unwrap();
        assert_eq!(None, loaded);
    }

    #[tokio::test]
    async fn save_new_file_and_load() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db = DatabaseFile::new(some_db_1(), tempfile.clone(), cipher(1));

        db.save().await.unwrap();
        let loaded = DatabaseFile::load(tempfile, cipher(1)).await.unwrap();
        assert_eq!(some_db_1(), *loaded.unwrap().database());
    }

    #[tokio::test]
    async fn overwrite_existing_file_and_load() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db1 = DatabaseFile::new(some_db_1(), tempfile.clone(), cipher(1));
        let db2 = DatabaseFile::new(some_db_2(), tempfile.clone(), cipher(1));

        db1.save().await.unwrap();
        db2.save().await.unwrap();
        let loaded = DatabaseFile::load(tempfile, cipher(1))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(some_db_1(), *loaded.database());
        assert_eq!(some_db_2(), *loaded.database());
    }

    #[tokio::test]
    async fn doesnt_load_with_wrong_key() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db = DatabaseFile::new(some_db_1(), tempfile.clone(), cipher(2));

        db.save().await.unwrap();
        let loaded = DatabaseFile::load(tempfile, cipher(1))
            .await
            .unwrap_err()
            .to_string();
        assert_eq!("aead::Error", loaded);
    }
}
