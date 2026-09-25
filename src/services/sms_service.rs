use serde::Deserialize;

use crate::error::{AppError, Result};

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
    amount_tetri: i32,
) -> Result<()> {
    let amount = format!("{}.{:02}", amount_tetri / 100, amount_tetri % 100);
    let content = format!("თქვენი შეკვეთა მიღებულია, თანხა {amount} ₾. გმადლობთ!");
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
