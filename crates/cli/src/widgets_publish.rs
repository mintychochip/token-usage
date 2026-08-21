//! Publish a signed usage summary to a widget service.

use base64::Engine;
use serde_json::{json, Value};
use toktally_store::FileStore;

use crate::{identity::load_or_generate, summarize_priced, PriceTable, UsageSummary};

/// Publish a pre-computed summary to a widget service.
///
/// Loads the local identity, signs the summary, POSTs it, and returns the public
/// summary URL for the user\'s UUID.
pub fn publish_summary(summary: &UsageSummary, service_url: &str) -> Result<String, String> {
    let id = load_or_generate().map_err(|e| format!("identity: {e}"))?;

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

/// Publish the current local store's aggregate summary to a widget service.
///
/// Returns the public summary URL for the user\'s UUID.
pub fn publish_to_widgets(
    store: &FileStore,
    service_url: &str,
    prices: Option<&PriceTable>,
) -> Result<String, String> {
    let listed = store.list().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let summary = summarize_priced(&listed, now, prices);
    publish_summary(&summary, service_url)
}

/// Verify a widget publish body returned from a client.
///
/// `body` is the full JSON request. This function removes the `signature` field,
/// verifies the remaining payload, and returns `(uuid, summary)` on success.
pub fn verify_publish_body(body: &Value) -> Result<(String, Value), String> {
    let mut signed = body.clone();
    let signature_b64 = signed
        .get("signature")
        .and_then(|v| v.as_str())
        .ok_or("missing signature")?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| e.to_string())?;
    signed
        .as_object_mut()
        .ok_or("body is not an object")?
        .remove("signature");

    let uuid = signed
        .get("uuid")
        .and_then(|v| v.as_str())
        .ok_or("missing uuid")?
        .to_string();

    let summary = signed
        .get("summary")
        .cloned()
        .ok_or("missing summary")?;

    let public_key_b64 = signed
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or("missing public_key")?;
    let public_key = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64)
        .map_err(|e| e.to_string())?;

    if !crate::identity::verify_json(&signed, &public_key, &signature)? {
        return Err("signature verification failed".to_string());
    }

    Ok((uuid, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_sign_and_verify_publish_body() {
        let identity = crate::identity::load_or_generate_in(
            &std::env::temp_dir().join(format!("toktally-widget-test-{}", std::process::id())),
        )
        .unwrap();

        let summary = UsageSummary {
            generated_at: 1,
            harnesses: vec![],
            input_tokens: 100,
            output_tokens: 50,
            estimated_cost_usd: None,
        };

        let mut body = json!({
            "uuid": identity.uuid,
            "public_key": base64::engine::general_purpose::STANDARD.encode(&identity.public_key),
            "summary": summary,
        });

        let signature = identity.sign_json(&body).unwrap();
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(&signature);
        body["signature"] = json!(signature_b64);

        let (verified_uuid, verified_summary) = verify_publish_body(&body).unwrap();
        assert_eq!(verified_uuid, identity.uuid);
        assert_eq!(verified_summary["input_tokens"], 100);
        assert_eq!(verified_summary["output_tokens"], 50);
    }

    #[test]
    fn verify_publish_body_rejects_tampered_summary() {
        let identity = crate::identity::load_or_generate_in(
            &std::env::temp_dir().join(format!("toktally-widget-test-{}", std::process::id())),
        )
        .unwrap();

        let summary = UsageSummary {
            generated_at: 1,
            harnesses: vec![],
            input_tokens: 100,
            output_tokens: 50,
            estimated_cost_usd: None,
        };

        let mut body = json!({
            "uuid": identity.uuid,
            "public_key": base64::engine::general_purpose::STANDARD.encode(&identity.public_key),
            "summary": summary,
        });

        let signature = identity.sign_json(&body).unwrap();
        let signature_b64 = base64::engine::general_purpose::STANDARD.encode(&signature);
        body["signature"] = json!(signature_b64);

        body["summary"]["input_tokens"] = 999.into();

        assert!(verify_publish_body(&body).is_err());
    }
}
