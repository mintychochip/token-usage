//! Machine-bound ed25519 identity for widget publishing.
//!
//! The private key never leaves the machine. The public key and a stable UUID
//! derived from it identify the user's widget.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_KEY_FILE: &str = "identity.pub";
const SECRET_KEY_FILE: &str = "identity.sec";

/// A loaded or generated identity.
pub struct Identity {
    /// Stable public identifier derived from the public key.
    pub uuid: String,
    /// 32-byte ed25519 public key.
    pub public_key: Vec<u8>,
    keypair: SigningKey,
}

impl Identity {
    /// Sign a JSON value by canonicalizing it to compact JSON first.
    pub fn sign_json(&self, value: &Value) -> Result<Vec<u8>, String> {
        let canonical = canonical_json(value)?;
        let signature = self.keypair.sign(&canonical);
        Ok(signature.to_bytes().to_vec())
    }
}

/// Return the default configuration directory, `~/.toktally` or the value of
/// `TOKTALLY_CONFIG_DIR`.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TOKTALLY_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("toktally");
    }
    home_dir().join(".toktally")
}

/// Return the directory that holds the identity keypair.
pub fn key_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TOKTALLY_IDENTITY_DIR") {
        return PathBuf::from(dir);
    }
    config_dir().join("keys")
}

/// Load an existing identity or generate a new one in the default key directory.
pub fn load_or_generate() -> Result<Identity, String> {
    load_or_generate_in(&key_dir())
}

/// Load an existing identity or generate a new one in `dir`.
pub fn load_or_generate_in(dir: &Path) -> Result<Identity, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;

    let pub_path = dir.join(PUBLIC_KEY_FILE);
    let sec_path = dir.join(SECRET_KEY_FILE);

    if pub_path.exists() && sec_path.exists() {
        let public_key = fs::read(&pub_path).map_err(|e| e.to_string())?;
        let secret_bytes = fs::read(&sec_path).map_err(|e| e.to_string())?;

        let secret_array: [u8; 32] = secret_bytes
            .try_into()
            .map_err(|_| "identity secret key has wrong length".to_string())?;

        let keypair = SigningKey::from_bytes(&secret_array);

        return Ok(Identity {
            uuid: uuid_from_public_key(&public_key),
            public_key,
            keypair,
        });
    }

    let mut rng = rand::rngs::OsRng;
    let keypair = SigningKey::generate(&mut rng);
    let public_key = keypair.verifying_key().to_bytes().to_vec();

    fs::write(&sec_path, keypair.to_bytes()).map_err(|e| e.to_string())?;
    fs::write(&pub_path, &public_key).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sec_path, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }

    Ok(Identity {
        uuid: uuid_from_public_key(&public_key),
        public_key,
        keypair,
    })
}

/// Verify a JSON value was signed by `public_key`.
pub fn verify_json(value: &Value, public_key: &[u8], signature: &[u8]) -> Result<bool, String> {
    let canonical = canonical_json(value)?;

    let public_array: &[u8; 32] = public_key
        .try_into()
        .map_err(|_| "identity public key has wrong length".to_string())?;

    let verifying_key = VerifyingKey::from_bytes(public_array)
        .map_err(|e| format!("invalid public key: {e}"))?;

    let sig_array: &[u8; 64] = signature
        .try_into()
        .map_err(|_| "signature has wrong length".to_string())?;

    let sig = Signature::from_bytes(sig_array);

    Ok(verifying_key.verify(&canonical, &sig).is_ok())
}

/// Derive a stable UUID from a public key by blake3-hashing it and formatting
/// the first 16 bytes as a version-4-looking UUID.
pub fn uuid_from_public_key(public_key: &[u8]) -> String {
    let hash = blake3::hash(public_key);
    let b = hash.as_bytes();

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_be_bytes([b[4], b[5]]),
        u16::from_be_bytes([b[6], b[7]]),
        u16::from_be_bytes([b[8], b[9]]),
        u64::from_be_bytes([b[10], b[11], b[12], b[13], b[14], b[15], 0, 0])
            & 0x0000ffffffffffff,
    )
}

fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home);
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(profile);
    }
    PathBuf::from(".")
}

fn canonical_json(value: &Value) -> Result<Vec<u8>, String> {
    // serde_json::to_vec preserves field order from the struct/Value construction.
    // For a generated summary this is stable enough; clients and server both
    // reconstruct the same Value before verifying.
    serde_json::to_vec(value).map_err(|e| e.to_string())
}
