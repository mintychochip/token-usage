//! Publish a signed usage summary to a widget service.

use base64::Engine;
use serde_json::{json, Value};
use token_usage_store::FileStore;

use crate::{identity::load_or_generate, summarize_priced, PriceTable};

/// Publish the current local store's aggregate summary to a widget service.
///
/// Returns the public summary URL for the user's UUID.
pub fn publish_to_widgets(
    store: &FileStore,
    service_url: &str,
    prices: Option<&PriceTable>,
) -> Result<String, String> {
    let id = load_or_generate().map_err(|e| format!("identity: {e}"))?;

    let listed = store.list().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let summary = summarize_priced(&listed, now, prices);

    let display_name = std::env::var("TOKTALLY_DISPLAY_NAME").ok();

    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(&id.public_key);

    let mut signed_body = json!({
        "uuid": id.uuid,
        "public_key": public_key_b64,
        "summary": summary,
        "display_name": display_name,
    });

    let signature = id.sign_json(&signed_body)?;
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(&signature);

    signed_body["signature"] = json!(signature_b64);

    let url = format!("{service_url}/api/v1/publish");
    let response = ureq::post(&url)
        .send_json(&signed_body)
        .map_err(|e| format!("POST failed: {e}"))?;

    let status = response.status();
    if status != 200 {
        return Err(format!("widget publish failed: HTTP {status}"));
    }

    Ok(format!("{service_url}/u/{}/usage-summary.json", id.uuid))
}

/// Verify a widget publish body returned from a client.
///
/// `body` is the full JSON request. This function removes the `signature` field,
/// verifies the remaining payload, and returns `(uuid, summary)` on success.
pub fn verify_publish_body(body: &Value) -> Result<(String, Value), String> {
    let uuid = body
        .get("uuid")
        .and_then(|v| v.as_str())
        .ok_or("missing uuid")?
        .to_string();

    let public_key_b64 = body
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or("missing public_key")?;
    let public_key = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| format!("bad public_key: {e}"))?;

    let signature_b64 = body
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or("missing signature")?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| format!("bad signature: {e}"))?;

    let summary = body
        .get("summary")
        .cloned()
        .ok_or("missing summary")?;

    let mut signed = body.clone();
    signed.as_object_mut().unwrap().remove("signature");
    if !crate::identity::verify_json(&signed, &public_key, &signature)? {
        return Err("signature verification failed".to_string());
    }

    Ok((uuid, summary))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::load_or_generate_in;
    use serde_json::json;

    #[test]
    fn round_trip_sign_and_verify_publish_body() {
        let tmp = tempfile::tempdir().unwrap();
        let id = load_or_generate_in(tmp.path()).unwrap();

        let summary = json!({
            "totals": { "input_tokens": 100, "output_tokens": 50 },
            "estimated_cost_usd": 1.23,
        });
        let display_name: Option<String> = None;
        let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(&id.public_key);

        let mut body = json!({
            "uuid": id.uuid,
            "public_key": public_key_b64,
            "summary": summary,
            "display_name": display_name,
        });

        let sig = id.sign_json(&body).unwrap();
        body["signature"] = json!(base64::engine::general_purpose::STANDARD.encode(&sig));

        let (uuid, got_summary) = verify_publish_body(&body).unwrap();
        assert_eq!(uuid, id.uuid);
        assert_eq!(got_summary, summary);
    }

    #[test]
    fn verify_publish_body_rejects_tampered_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let id = load_or_generate_in(tmp.path()).unwrap();

        let summary = json!({ "totals": { "input_tokens": 100 } });
        let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(&id.public_key);

        let mut body = json!({
            "uuid": id.uuid,
            "public_key": public_key_b64,
            "summary": summary,
            "display_name": serde_json::Value::Null,
        });

        let sig = id.sign_json(&body).unwrap();
        body["signature"] = json!(base64::engine::general_purpose::STANDARD.encode(&sig));

        // Tamper with summary
        body["summary"]["totals"]["input_tokens"] = json!(999);

        assert!(verify_publish_body(&body).is_err());
    }
}
