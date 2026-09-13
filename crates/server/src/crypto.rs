use std::sync::Arc;

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::{self, SaltString},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(crate) const PASSWORD_MEMORY_KIB: u32 = 19_456;
pub(crate) const PASSWORD_ITERATIONS: u32 = 2;
pub(crate) const PASSWORD_LANES: u32 = 1;
pub(crate) const PASSWORD_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub(crate) enum CryptoError {
    #[error("password operation capacity exhausted")]
    Busy,
    #[error("operating-system randomness unavailable")]
    Entropy,
    #[error("password hashing failed")]
    Hash,
    #[error("stored password hash is invalid")]
    InvalidStoredHash,
    #[error("password worker failed")]
    Worker,
}

#[derive(Clone)]
pub(crate) struct Passwords {
    permits: Arc<Semaphore>,
    dummy_hash: Arc<str>,
}

impl Passwords {
    pub(crate) async fn new() -> Result<Self, CryptoError> {
        let permits = Arc::new(Semaphore::new(PASSWORD_CONCURRENCY));
        let permit = permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| CryptoError::Busy)?;
        let dummy_password = SessionToken::generate()?.raw;
        let dummy_hash = hash_in_worker(dummy_password, permit).await?;
        Ok(Self {
            permits,
            dummy_hash: dummy_hash.into(),
        })
    }

    fn admit(&self) -> Result<OwnedSemaphorePermit, CryptoError> {
        self.permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| CryptoError::Busy)
    }

    pub(crate) async fn hash(&self, password: String) -> Result<String, CryptoError> {
        hash_in_worker(password, self.admit()?).await
    }

    pub(crate) async fn verify(
        &self,
        password: String,
        stored_hash: Option<String>,
    ) -> Result<bool, CryptoError> {
        let permit = self.admit()?;
        let hash = stored_hash.unwrap_or_else(|| self.dummy_hash.to_string());
        tokio::task::spawn_blocking(move || {
            // Cancellation must not release capacity while this worker is still hashing.
            let _permit = permit;
            verify_sync(&password, &hash)
        })
        .await
        .map_err(|_| CryptoError::Worker)?
    }
}

async fn hash_in_worker(
    password: String,
    permit: OwnedSemaphorePermit,
) -> Result<String, CryptoError> {
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_sync(&password)
    })
    .await
    .map_err(|_| CryptoError::Worker)?
}

fn argon2() -> Result<Argon2<'static>, CryptoError> {
    let params = Params::new(
        PASSWORD_MEMORY_KIB,
        PASSWORD_ITERATIONS,
        PASSWORD_LANES,
        Some(32),
    )
    .map_err(|_| CryptoError::Hash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn hash_sync(password: &str) -> Result<String, CryptoError> {
    let mut salt = [0_u8; 16];
    getrandom::fill(&mut salt).map_err(|_| CryptoError::Entropy)?;
    let salt = SaltString::encode_b64(&salt).map_err(|_| CryptoError::Hash)?;
    argon2()?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| CryptoError::Hash)
}

fn verify_sync(password: &str, encoded: &str) -> Result<bool, CryptoError> {
    let hash = PasswordHash::new(encoded).map_err(|_| CryptoError::InvalidStoredHash)?;
    // Stored data must not turn a bounded password worker into an unbounded allocation.
    if hash.algorithm.as_str() != "argon2id"
        || hash.version != Some(19)
        || hash.params.get_decimal("m") != Some(PASSWORD_MEMORY_KIB)
        || hash.params.get_decimal("t") != Some(PASSWORD_ITERATIONS)
        || hash.params.get_decimal("p") != Some(PASSWORD_LANES)
        || hash.hash.is_none_or(|output| output.len() != 32)
    {
        return Err(CryptoError::InvalidStoredHash);
    }
    match argon2()?.verify_password(password.as_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(password_hash::Error::Password) => Ok(false),
        Err(_) => Err(CryptoError::InvalidStoredHash),
    }
}

pub(crate) struct SessionToken {
    pub(crate) raw: String,
    pub(crate) digest: [u8; 32],
}

impl SessionToken {
    pub(crate) fn generate() -> Result<Self, CryptoError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).map_err(|_| CryptoError::Entropy)?;
        let raw = URL_SAFE_NO_PAD.encode(bytes);
        let digest = Sha256::digest(raw.as_bytes()).into();
        Ok(Self { raw, digest })
    }
}

pub(crate) fn token_digest(token: &str) -> Option<[u8; 32]> {
    if token.len() != 43 {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(token).ok()?;
    if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(decoded) != token {
        return None;
    }
    Some(Sha256::digest(token.as_bytes()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_use_argon2id_with_independent_salts_and_preserve_bytes() {
        let password = "  Case-sensitive pénguin password  ";
        let first = hash_sync(password).unwrap();
        let second = hash_sync(password).unwrap();
        assert_ne!(first, second);
        assert!(!first.contains(password));
        let parsed = PasswordHash::new(&first).unwrap();
        assert_eq!(parsed.algorithm.as_str(), "argon2id");
        assert_eq!(parsed.version, Some(19));
        assert_eq!(parsed.params.get_decimal("m"), Some(19_456));
        assert_eq!(parsed.params.get_decimal("t"), Some(2));
        assert_eq!(parsed.params.get_decimal("p"), Some(1));
        assert_ne!(parsed.salt, PasswordHash::new(&second).unwrap().salt);
        assert!(verify_sync(password, &first).unwrap());
        assert!(!verify_sync(password.trim(), &first).unwrap());
        assert!(!verify_sync(&password.to_uppercase(), &first).unwrap());
    }

    #[tokio::test]
    async fn password_workers_reject_a_fifth_operation_and_unknown_users_use_a_hash() {
        let passwords = Passwords::new().await.unwrap();
        assert!(PasswordHash::new(&passwords.dummy_hash).is_ok());
        assert!(
            !passwords
                .verify("unknown-password".into(), None)
                .await
                .unwrap()
        );
        let permits: Vec<_> = (0..PASSWORD_CONCURRENCY)
            .map(|_| passwords.admit().unwrap())
            .collect();
        assert!(matches!(
            passwords.verify("password".into(), None).await,
            Err(CryptoError::Busy)
        ));
        assert!(matches!(
            passwords.hash("password".into()).await,
            Err(CryptoError::Busy)
        ));
        drop(permits);
        assert!(passwords.admit().is_ok());
    }

    #[test]
    fn corrupt_or_excessive_stored_parameters_are_internal_errors_not_bad_credentials() {
        let valid = hash_sync("a long test password").unwrap();
        for invalid in [
            "not-a-hash".to_owned(),
            valid.replace("argon2id", "argon2i"),
            valid.replace("m=19456", "m=4294967295"),
            valid.replace("t=2", "t=1"),
            valid.replace("p=1", "p=2"),
        ] {
            assert!(matches!(
                verify_sync("a long test password", &invalid),
                Err(CryptoError::InvalidStoredHash)
            ));
        }
    }

    #[test]
    fn tokens_are_canonical_256_bit_random_values_and_only_digests_are_used_for_lookup() {
        let first = SessionToken::generate().unwrap();
        let second = SessionToken::generate().unwrap();
        assert_eq!(first.raw.len(), 43);
        assert_eq!(URL_SAFE_NO_PAD.decode(&first.raw).unwrap().len(), 32);
        assert_ne!(first.raw, second.raw);
        assert_ne!(first.digest, second.digest);
        assert_eq!(token_digest(&first.raw), Some(first.digest));
        for invalid in [
            String::new(),
            "a".repeat(42),
            "a".repeat(44),
            "!".repeat(43),
            format!("{}=", first.raw),
            format!(" {}", first.raw),
            format!("{} ", first.raw),
            "é".repeat(22),
            format!("{}B", "A".repeat(42)),
        ] {
            assert!(token_digest(&invalid).is_none());
        }
    }
}
