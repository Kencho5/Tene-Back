use serde::Deserialize;

use crate::error::{AppError, Result};
use crate::models::Order;

const SMS_SEND_URL: &str = "https://smsoffice.ge/api/v2/send/";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SmsResponse {
    success: bool,
    message: Option<String>,
    error_code: i32,
}

pub async fn send_verification_code(
    api_key: &str,
    sender: &str,
    destination: &str,
    code: i32,
) -> Result<()> {
    let content = format!("თქვენი დამადასტურებელი კოდია {code}");
    send_sms(api_key, sender, destination, &content).await
}

pub async fn send_order_confirmation(
    api_key: &str,
    sender: &str,
    destination: &str,
    order: &Order,
) -> Result<()> {
    let amount = format!("{}.{:02}", order.amount / 100, order.amount % 100);
    let delivery = match (order.delivery_type.as_str(), order.delivery_time.as_str()) {
        ("pickup", _) => "თვითგატანა".to_string(),
        (_, "same_day") => "მიწოდება იმავე დღეს".to_string(),
        (_, "next_day") => "მიწოდება მეორე დღეს".to_string(),
        _ => "მიწოდება".to_string(),
    };
    let content = format!(
        "შეკვეთა #{} მიღებულია!\nთანხა: {amount} ₾\n{delivery}\nდეტალები გამოგზავნილია ელფოსტაზე. გმადლობთ!",
        order.order_id
    );
    send_sms(api_key, sender, destination, &content).await
}

async fn send_sms(api_key: &str, sender: &str, destination: &str, content: &str) -> Result<()> {
    let response = reqwest::Client::new()
        .post(SMS_SEND_URL)
        .form(&[
            ("key", api_key),
            ("destination", destination),
            ("sender", sender),
            ("content", content),
        ])
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("SMS მოთხოვნა ვერ მოხერხდა: {e}")))?;

    let body: SmsResponse = response
        .json()
        .await
        .map_err(|e| AppError::InternalError(format!("SMS პასუხის გარჩევა ვერ მოხერხდა: {e}")))?;

    if !body.success || body.error_code != 0 {
        tracing::error!(
            "SMS send failed: code={} message={:?}",
            body.error_code,
            body.message
        );
        return Err(match body.error_code {
            10 | 75 | 76 | 77 => {
                AppError::BadRequest("ამ ნომერზე SMS-ის გაგზავნა ვერ მოხერხდა".to_string())
            }
            _ => AppError::InternalError("SMS-ის გაგზავნა ვერ მოხერხდა".to_string()),
        });
    }

    Ok(())
}
