use anyhow::{anyhow, ensure, Result};
use crc::{Crc, CRC_32_BZIP2};
use std::path::Path;

use super::{crypto::Cipher, database::DatabaseV1, Database};

fn crc() -> Crc<u32> {
    // TODO Which crc algorithm should we use?
    Crc::<u32>::new(&CRC_32_BZIP2)
}

/// Returns Ok(None) if the db file doesn't exist yet
pub async fn load(path: &Path, cipher: &impl Cipher) -> Result<Option<DatabaseV1>> {
    log::info!("Loading database...");
    if !tokio::fs::try_exists(path).await? {
        return Ok(None);
    }

    let content_ciphertext = tokio::fs::read(path).await?;
    let content_plaintext = cipher.decrypt(&content_ciphertext)?;
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

    Ok(Some(database))
}

pub async fn save(db: DatabaseV1, path: &Path, cipher: &impl Cipher) -> Result<()> {
    log::info!("Saving database...");

    let crc = crc();
    let content_plaintext = postcard::to_stdvec_crc32(&Database::V1(db), crc.digest())?;
    let content_compressed = zstd::bulk::compress(
        &content_plaintext,
        zstd::compression_level_range().last().unwrap(),
    )?;
    let content_ciphertext = cipher.encrypt(&content_compressed)?;

    // First write to temporary file so we don't lose data if writing fails halfway
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow!("Path has no filename"))?
        .to_str()
        .ok_or_else(|| anyhow!("Filename isn't valid utf-8"))?;
    let tmppath = path.with_file_name(format!("{}.temp:", filename));
    tokio::fs::write(&tmppath, content_ciphertext).await?;

    // Ok, writing succeeded, let's now replace the real file with the tmpfile
    tokio::fs::rename(&tmppath, path).await?;

    log::info!("Saving database...done");

    Ok(())
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

    fn cipher(seed: u64) -> impl Cipher {
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

        let loaded = load(&tempfile, &cipher(1)).await.unwrap();
        assert_eq!(None, loaded);
    }

    #[tokio::test]
    async fn save_new_file_and_load() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db = some_db_1();

        save(db.clone(), &tempfile, &cipher(1)).await.unwrap();
        let loaded = load(&tempfile, &cipher(1)).await.unwrap();
        assert_eq!(db, loaded.unwrap());
    }

    #[tokio::test]
    async fn overwrite_existing_file_and_load() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db1 = some_db_1();
        let db2 = some_db_2();

        save(db1.clone(), &tempfile, &cipher(1)).await.unwrap();
        save(db2.clone(), &tempfile, &cipher(1)).await.unwrap();
        let loaded = load(&tempfile, &cipher(1)).await.unwrap().unwrap();
        assert_ne!(db1, loaded);
        assert_eq!(db2, loaded);
    }

    #[tokio::test]
    async fn doesnt_load_with_wrong_key() {
        let tempdir = tempfile::tempdir().unwrap();
        let tempfile = tempdir.path().join("database");

        let db = some_db_1();

        save(db.clone(), &tempfile, &cipher(2)).await.unwrap();
        let loaded = load(&tempfile, &cipher(1)).await.unwrap_err().to_string();
        assert_eq!("aead::Error", loaded);
    }
}
