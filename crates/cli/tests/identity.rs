use serde_json::json;
use toktally_cli::identity::{load_or_generate_in, uuid_from_public_key, verify_json};

#[test]
fn identity_generates_keys_and_uuid() {
    let tmp = tempfile::tempdir().unwrap();
    let id = load_or_generate_in(tmp.path()).unwrap();

    assert!(!id.uuid.is_empty());
    assert_eq!(id.public_key.len(), 32);
    assert!(tmp.path().join("identity.pub").exists());
    assert!(tmp.path().join("identity.sec").exists());
}

#[test]
fn identity_is_stable_across_loads() {
    let tmp = tempfile::tempdir().unwrap();
    let a = load_or_generate_in(tmp.path()).unwrap();
    let b = load_or_generate_in(tmp.path()).unwrap();

    assert_eq!(a.uuid, b.uuid);
    assert_eq!(a.public_key, b.public_key);
}

#[test]
fn sign_and_verify_json() {
    let tmp = tempfile::tempdir().unwrap();
    let id = load_or_generate_in(tmp.path()).unwrap();
    let value = json!({ "total": 1234 });

    let sig = id.sign_json(&value).unwrap();
    assert!(verify_json(&value, &id.public_key, &sig).unwrap());
}

#[test]
fn verify_rejects_tampered_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let id = load_or_generate_in(tmp.path()).unwrap();
    let value = json!({ "total": 1234 });

    let sig = id.sign_json(&value).unwrap();
    let tampered = json!({ "total": 9999 });
    assert!(!verify_json(&tampered, &id.public_key, &sig).unwrap());
}

#[test]
fn verify_rejects_wrong_public_key() {
    let tmp = tempfile::tempdir().unwrap();
    let a = load_or_generate_in(tmp.path()).unwrap();

    let other_dir = tmp.path().join("other");
    std::fs::create_dir(&other_dir).unwrap();
    let b = load_or_generate_in(&other_dir).unwrap();

    let value = json!({ "total": 1 });
    let sig = a.sign_json(&value).unwrap();
    assert!(!verify_json(&value, &b.public_key, &sig).unwrap());
}

#[test]
fn uuid_from_public_key_is_stable() {
    let a = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let b = a.clone();
    assert_eq!(uuid_from_public_key(&a), uuid_from_public_key(&b));
}
