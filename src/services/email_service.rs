use aws_sdk_sesv2::Client as SesClient;

use crate::error::{AppError, Result};

pub async fn send_verification_email(
    ses_client: &SesClient,
    recipient: &str,
    code: i32,
    sender_email: &str,
) -> Result<()> {
    let html_template = include_str!("../utils/code.html");
    let html = html_template.replace("{{verification_code}}", &code.to_string());

    let destination = aws_sdk_sesv2::types::Destination::builder()
        .to_addresses(recipient)
        .build();

    let subject = aws_sdk_sesv2::types::Content::builder()
        .data("Verify Your Email")
        .charset("UTF-8")
        .build()
        .map_err(|e| AppError::InternalError(format!("სათაურის აგება ვერ მოხერხდა: {}", e)))?;

    let html_body = aws_sdk_sesv2::types::Content::builder()
        .data(html)
        .charset("UTF-8")
        .build()
        .map_err(|e| AppError::InternalError(format!("HTML ტექსტის აგება ვერ მოხერხდა: {}", e)))?;

    let body = aws_sdk_sesv2::types::Body::builder()
        .html(html_body)
        .build();

    let message = aws_sdk_sesv2::types::Message::builder()
        .subject(subject)
        .body(body)
        .build();

    let content = aws_sdk_sesv2::types::EmailContent::builder()
        .simple(message)
        .build();

    ses_client
        .send_email()
        .from_email_address(sender_email)
        .destination(destination)
        .content(content)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to send email: {:?}", e);
            AppError::InternalError("დამადასტურებელი ელფოსტის გაგზავნა ვერ მოხერხდა".to_string())
        })?;

    Ok(())
}
