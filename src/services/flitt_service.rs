use std::collections::BTreeMap;

use sha1::{Digest, Sha1};

use crate::error::{AppError, Result};

const FLITT_CHECKOUT_URL: &str = "https://pay.flitt.com/api/checkout/url";

pub fn generate_signature(secret_key: &str, params: &BTreeMap<String, String>) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(params.len() + 1);
    parts.push(secret_key);

    for value in params.values() {
        if !value.is_empty() {
            parts.push(value);
        }
    }

    let joined = parts.join("|");
    tracing::debug!("Flitt signature string: {}", joined);
    let mut hasher = Sha1::new();
    hasher.update(joined.as_bytes());
    let sig = format!("{:x}", hasher.finalize());
    tracing::debug!("Flitt generated signature: {}", sig);
    sig
}

pub fn verify_callback_signature(
    secret_key: &str,
    params: &serde_json::Value,
) -> bool {
    let obj = match params.as_object() {
        Some(obj) => obj,
        None => return false,
    };

    let received_signature = match obj.get("signature").and_then(|v| v.as_str()) {
        Some(sig) => sig,
        None => return false,
    };

    let mut sorted: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in obj {
        if key == "signature" || key == "response_signature_string" {
            continue;
        }
        let str_value = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => continue,
        };
        if !str_value.is_empty() {
            sorted.insert(key.clone(), str_value);
        }
    }

    let generated = generate_signature(secret_key, &sorted);
    generated == received_signature
}

pub async fn create_checkout_url(
    merchant_id: i32,
    secret_key: &str,
    order_id: &str,
    amount: i32,
    order_desc: &str,
    server_callback_url: &str,
    response_url: &str,
) -> Result<String> {
    let mut params = BTreeMap::new();
    params.insert("amount".to_string(), amount.to_string());
    params.insert("currency".to_string(), "GEL".to_string());
    params.insert("merchant_id".to_string(), merchant_id.to_string());
    params.insert("order_desc".to_string(), order_desc.to_string());
    params.insert("order_id".to_string(), order_id.to_string());
    params.insert("response_url".to_string(), response_url.to_string());
    params.insert(
        "server_callback_url".to_string(),
        server_callback_url.to_string(),
    );
    params.insert("version".to_string(), "1.0.1".to_string());

    let signature = generate_signature(secret_key, &params);

    let request_body = serde_json::json!({
        "request": {
            "version": "1.0.1",
            "merchant_id": merchant_id,
            "order_id": order_id,
            "order_desc": order_desc,
            "amount": amount,
            "currency": "GEL",
            "response_url": response_url,
            "server_callback_url": server_callback_url,
            "signature": signature,
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(FLITT_CHECKOUT_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("Flitt API request failed: {}", e)))?;

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse Flitt response: {}", e)))?;

    let response_obj = body
        .get("response")
        .ok_or_else(|| AppError::InternalError("Invalid Flitt response format".to_string()))?;

    let response_status = response_obj
        .get("response_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if response_status != "success" {
        tracing::error!("Flitt API error response: {}", response_obj);
        let error_message = response_obj
            .get("error_message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Flitt error");
        return Err(AppError::InternalError(format!(
            "Flitt order creation failed: {}",
            error_message
        )));
    }

    let checkout_url = response_obj
        .get("checkout_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::InternalError("Flitt response missing checkout_url".to_string())
        })?;

    Ok(checkout_url.to_string())
}
