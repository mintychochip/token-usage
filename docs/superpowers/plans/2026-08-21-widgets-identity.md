# Widget Identity — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add automatic ed25519 identity key generation and signing to the token-usage reporter so each machine gets a stable UUID and can sign widget summaries without user setup.

**Architecture:** A new `identity` module in `crates/cli` handles key generation, persistence, UUID derivation, and signing. It uses `ed25519-dalek` for signing and `blake3` for stable UUID derivation from the public key. Keys live in `~/.toktally/keys/`.

**Tech Stack:** Rust, `ed25519-dalek`, `blake3`, existing `token-usage-cli` crate.

## Global Constraints

- Keep the CLI as a single binary; no new daemon.
- Identity generation must be automatic and silent on first publish.
- Private key never leaves the local machine.
- UUID must be stable for the same public key.
- All new code must have tests in `crates/cli/tests/`.
- No external network calls in this module.

---

### Task 1: Add dependencies

**Files:**
- Modify: `crates/cli/Cargo.toml`

**Interfaces:**
- Consumes: existing Cargo workspace.
- Produces: `ed25519-dalek` and `blake3` available to `crates/cli`.

- [ ] **Step 1: Add crates to `Cargo.toml`**

```toml
[dependencies]
ed25519-dalek = { version = "2.1.1", features = ["rand_core"] }
blake3 = "1.5.1"
serde_json = { workspace = true }
```

- [ ] **Step 2: Run `cargo check -p token-usage-cli` to confirm dependency resolution**

Run:
```bash
cargo check -p token-usage-cli
```

Expected: passes, no compile errors.

- [ ] **Step 3: Commit**

```bash
git add crates/cli/Cargo.toml
git commit -m "build(cli): add ed25519-dalek and blake3 for widget identity"
```

---

### Task 2: Create the identity module

**Files:**
- Create: `crates/cli/src/identity.rs`
- Modify: `crates/cli/src/lib.rs`

**Interfaces:**
- Consumes: `ed25519-dalek` key types, `blake3` hashing, `serde_json`.
- Produces:
  - `pub struct Identity { pub uuid: String, pub public_key: Vec<u8>, keypair: Keypair }`
  - `pub fn config_dir() -> PathBuf`
  - `pub fn key_dir() -> PathBuf`
  - `pub fn load_or_generate() -> Result<Identity, String>`
  - `pub fn sign_json(&self, value: &serde_json::Value) -> Result<Vec<u8>, String>`
  - `pub fn verify_json(value: &serde_json::Value, public_key: &[u8], signature: &[u8]) -> Result<bool, String>`
  - `pub fn uuid_from_public_key(public_key: &[u8]) -> String`

- [ ] **Step 1: Write the failing tests**

Create `crates/cli/tests/identity.rs`:

```rust
use std::fs;
use token_usage_cli::identity::{load_or_generate, verify_json, uuid_from_public_key};

#[test]
fn identity_generates_keys_and_uuid() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("TOKTALLY_IDENTITY_DIR", tmp.path().join("keys"));
    let id = load_or_generate().unwrap();
    assert!(!id.uuid.is_empty());
    assert!(!id.public_key.is_empty());
    assert!(tmp.path().join("keys/identity.pub").exists());
    assert!(tmp.path().join("keys/identity.sec").exists());
}

#[test]
fn identity_is_stable_across_loads() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("TOKTALLY_IDENTITY_DIR", tmp.path().join("keys"));
    let a = load_or_generate().unwrap();
    let b = load_or_generate().unwrap();
    assert_eq!(a.uuid, b.uuid);
    assert_eq!(a.public_key, b.public_key);
}

#[test]
fn sign_and_verify_json() {
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("TOKTALLY_IDENTITY_DIR", tmp.path().join("keys"));
    let id = load_or_generate().unwrap();
    let value = serde_json::json!({"total": 1234});
    let sig = id.sign_json(&value).unwrap();
    assert!(verify_json(&value, &id.public_key, &sig).unwrap());
}

#[test]
fn uuid_derives_from_public_key() {
    let a = vec![1u8, 2, 3, 4];
    let b = vec![1u8, 2, 3, 4];
    assert_eq!(uuid_from_public_key(&a), uuid_from_public_key(&b));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```bash
cargo test -p token-usage-cli identity
```

Expected: compile errors because `identity` module does not exist.

- [ ] **Step 3: Implement `crates/cli/src/identity.rs`**

```rust
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Identity {
    pub uuid: String,
    pub public_key: Vec<u8>,
    keypair: SigningKey,
}

impl Identity {
    pub fn sign_json(&self, value: &Value) -> Result<Vec<u8>, String> {
        let canonical = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        Ok(self.keypair.sign(&canonical).to_bytes().to_vec())
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TOKTALLY_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".toktally")
}

pub fn key_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TOKTALLY_IDENTITY_DIR") {
        return PathBuf::from(dir);
    }
    config_dir().join("keys")
}

pub fn load_or_generate() -> Result<Identity, String> {
    let dir = key_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let pub_path = dir.join("identity.pub");
    let sec_path = dir.join("identity.sec");

    if pub_path.exists() && sec_path.exists() {
        let public_key = fs::read(&pub_path).map_err(|e| e.to_string())?;
        let secret_bytes = fs::read(&sec_path).map_err(|e| e.to_string())?;
        let keypair = SigningKey::from_bytes(
            &secret_bytes
                .try_into()
                .map_err(|_| "invalid secret key length".to_string())?,
        );
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
    fs::set_permissions(&sec_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;

    Ok(Identity {
        uuid: uuid_from_public_key(&public_key),
        public_key,
        keypair,
    })
}

pub fn verify_json(value: &Value, public_key: &[u8], signature: &[u8]) -> Result<bool, String> {
    let canonical = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let verifying_key = VerifyingKey::from_bytes(
        public_key
            .try_into()
            .map_err(|_| "invalid public key length".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let sig = Signature::from_bytes(
        signature
            .try_into()
            .map_err(|_| "invalid signature length".to_string())?,
    );
    Ok(verifying_key.verify(&canonical, &sig).is_ok())
}

pub fn uuid_from_public_key(public_key: &[u8]) -> String {
    let hash = blake3::hash(public_key);
    let bytes = hash.as_bytes();
    uuid::Uuid::from_slice(&bytes[..16])
        .map(|u| u.to_string())
        .unwrap_or_else(|_| hex::encode(&bytes[..16]))
}
```

- [ ] **Step 4: Update `crates/cli/src/lib.rs`**

Add:
```rust
pub mod identity;
```

- [ ] **Step 5: Add `uuid` and `hex` and `rand` and `dirs` to `Cargo.toml`**

```toml
[dependencies]
...
uuid = { version = "1.10.0", features = ["v4"] }
hex = "0.4.3"
rand = "0.8.5"
dirs = "5.0.1"
```

- [ ] **Step 6: Run tests**

Run:
```bash
cargo test -p token-usage-cli identity
```

Expected: all 4 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/cli/src/identity.rs crates/cli/src/lib.rs crates/cli/tests/identity.rs crates/cli/Cargo.toml
git commit -m "feat(cli): automatic ed25519 identity with stable UUID"
```

---

### Task 3: Expose identity in reporter CLI

**Files:**
- Modify: `crates/cli/src/bin/token-usage-reporter.rs`

**Interfaces:**
- Consumes: `token_usage_cli::identity::load_or_generate`.
- Produces: `reporter identity` subcommand that prints uuid and public key.

- [ ] **Step 1: Add an `identity` command to the CLI**

In `crates/cli/src/bin/token-usage-reporter.rs`, add to the `Command` enum:

```rust
Identity {
    #[arg(long)]
    show_secret: bool,
}
```

And handle it by loading the identity and printing:

```rust
Command::Identity { show_secret } => {
    let id = token_usage_cli::identity::load_or_generate()?;
    println!("uuid: {}", id.uuid);
    println!("public_key: {}", base64::encode(&id.public_key));
    if show_secret {
        println!("secret_path: {}", token_usage_cli::identity::key_dir().join("identity.sec").display());
    }
}
```

- [ ] **Step 2: Add `base64` dependency to `Cargo.toml`**

```toml
base64 = "0.22.1"
```

- [ ] **Step 3: Run smoke test**

Run:
```bash
cargo run -p token-usage-cli --bin token-usage-reporter -- identity
```

Expected: prints a uuid and public key, creates `~/.toktally/keys/` if absent.

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/bin/token-usage-reporter.rs crates/cli/Cargo.toml
git commit -m "feat(cli): add identity subcommand for widget publish"
```

---

## Spec Coverage

| Spec requirement | Task |
|---|---|
| ed25519 keypair in `~/.toktally/keys/` | Task 2 |
| Automatic generation on first use | Task 2 |
| Stable UUID from public key | Task 2 |
| Signing JSON summaries | Task 2 |
| CLI exposure for users | Task 3 |
