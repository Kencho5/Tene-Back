use aws_sdk_sesv2::Client as SesClient;
use rust_decimal::{Decimal, prelude::ToPrimitive};

use crate::{
    error::{AppError, Result},
    models::{Order, OrderItem},
};

const SENDER_EMAIL: &str = "Tene <support@tene.ge>";

pub async fn send_verification_email(
    ses_client: &SesClient,
    recipient: &str,
    code: i32,
    sender_email: &str,
) -> Result<()> {
    let html = include_str!("../utils/code.html")
        .replace("{{verification_code}}", &code.to_string());

    send_email(ses_client, sender_email, recipient, "Verify Your Email", &html).await
}

pub async fn send_order_confirmation_email(
    ses_client: &SesClient,
    order: &Order,
    items: &[OrderItem],
) -> Result<()> {
    let html = render_order_confirmation(order, items);
    let subject = format!("შეკვეთა {} მიღებულია", order.order_id);

    send_email(ses_client, SENDER_EMAIL, &order.email, &subject, &html).await
}

fn render_order_confirmation(order: &Order, items: &[OrderItem]) -> String {
    let mut rows = String::new();
    let mut subtotal = Decimal::ZERO;
    for item in items {
        let line_total = item.price_at_purchase * Decimal::from(item.quantity);
        subtotal += line_total;

        let mut meta_parts: Vec<String> = Vec::new();
        meta_parts.push(format!("რაოდენობა: {}", item.quantity));
        if let Some(color) = &item.color {
            meta_parts.push(format!("ფერი: {}", html_escape(color)));
        }
        if let Some(cfg) = &item.cable_config {
            let watts = cfg.get("watts").and_then(|v| v.as_i64());
            let length = cfg.get("length_cm").and_then(|v| v.as_i64());
            if let (Some(w), Some(l)) = (watts, length) {
                meta_parts.push(format!("{}W · {}სმ", w, l));
            }
        }
        let meta = meta_parts.join(" · ");

        let unit_price = format!("{} ₾ × {}", format_money(item.price_at_purchase), item.quantity);

        rows.push_str(&format!(
            "<tr>\
                <td>\
                    <div class=\"item-name\">{name}</div>\
                    <div class=\"item-meta\">{meta}</div>\
                    <div class=\"item-unit\">{unit}</div>\
                </td>\
                <td class=\"item-price\">{total} ₾</td>\
             </tr>",
            name = html_escape(&item.product_name),
            meta = meta,
            unit = unit_price,
            total = format_money(line_total),
        ));
    }

    let total_gel = Decimal::from(order.amount) / Decimal::from(100);
    let delivery_amount = total_gel - subtotal;
    let delivery_price = if delivery_amount <= Decimal::ZERO {
        "უფასო".to_string()
    } else {
        format!("{} ₾", format_money(delivery_amount))
    };

    let customer = if order.customer_type == "company" {
        order.organization_name.clone().unwrap_or_default()
    } else {
        format!(
            "{} {}",
            order.customer_name.as_deref().unwrap_or(""),
            order.customer_surname.as_deref().unwrap_or("")
        )
        .trim()
        .to_string()
    };

    let organization_block = if order.customer_type == "company" {
        let mut html = String::new();
        if let Some(t) = &order.organization_type {
            html.push_str(&format!(
                "<div class=\"info-row\"><strong>ორგ. ტიპი:</strong> {}</div>",
                html_escape(t)
            ));
        }
        if let Some(c) = &order.organization_code {
            html.push_str(&format!(
                "<div class=\"info-row\"><strong>ს/კ:</strong> {}</div>",
                html_escape(c)
            ));
        }
        html
    } else {
        String::new()
    };

    let delivery_type_label = match order.delivery_type.as_str() {
        "pickup" => "თვითგატანა",
        "courier" => "კურიერი",
        other => other,
    };

    let address_block = if order.delivery_type == "pickup" {
        String::new()
    } else {
        let mut html = String::new();
        if let Some(city) = &order.city {
            html.push_str(&format!(
                "<div class=\"info-row\"><strong>ქალაქი:</strong> {}</div>",
                html_escape(city)
            ));
        }
        html.push_str(&format!(
            "<div class=\"info-row\"><strong>მისამართი:</strong> {}</div>",
            html_escape(&order.address)
        ));
        if let Some(details) = &order.details {
            if !details.is_empty() {
                html.push_str(&format!(
                    "<div class=\"info-row\"><strong>დეტალები:</strong> {}</div>",
                    html_escape(details)
                ));
            }
        }
        html
    };

    let comment_block = order
        .comment
        .as_deref()
        .filter(|c| !c.trim().is_empty())
        .map(|c| {
            format!(
                "<div class=\"comment\"><strong>კომენტარი</strong>{}</div>",
                html_escape(c)
            )
        })
        .unwrap_or_default();

    let payment_id = order
        .payment_id
        .map(|p| p.to_string())
        .unwrap_or_else(|| "—".to_string());

    let created_at = order.created_at.format("%Y-%m-%d %H:%M UTC").to_string();

    include_str!("../utils/order_confirmation.html")
        .replace("{{order_id}}", &html_escape(&order.order_id))
        .replace("{{created_at}}", &created_at)
        .replace("{{items_rows}}", &rows)
        .replace("{{subtotal}}", &format_money(subtotal))
        .replace("{{delivery_price}}", &delivery_price)
        .replace("{{total}}", &format_money(total_gel))
        .replace("{{customer}}", &html_escape(&customer))
        .replace("{{email}}", &html_escape(&order.email))
        .replace("{{phone}}", &html_escape(&order.phone_number))
        .replace("{{organization_block}}", &organization_block)
        .replace("{{delivery_type}}", delivery_type_label)
        .replace("{{delivery_time}}", &html_escape(&order.delivery_time))
        .replace("{{address_block}}", &address_block)
        .replace("{{comment_block}}", &comment_block)
        .replace("{{currency}}", &html_escape(&order.currency))
        .replace("{{payment_id}}", &html_escape(&payment_id))
}

fn format_money(amount: Decimal) -> String {
    amount
        .to_f64()
        .map(|v| format!("{:.2}", v))
        .unwrap_or_else(|| amount.to_string())
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

async fn send_email(
    ses_client: &SesClient,
    sender: &str,
    recipient: &str,
    subject: &str,
    html: &str,
) -> Result<()> {
    let destination = aws_sdk_sesv2::types::Destination::builder()
        .to_addresses(recipient)
        .build();

    let subject = aws_sdk_sesv2::types::Content::builder()
        .data(subject)
        .charset("UTF-8")
        .build()
        .map_err(|e| AppError::InternalError(format!("სათაურის აგება ვერ მოხერხდა: {}", e)))?;

    let html_body = aws_sdk_sesv2::types::Content::builder()
        .data(html)
        .charset("UTF-8")
        .build()
        .map_err(|e| AppError::InternalError(format!("HTML ტექსტის აგება ვერ მოხერხდა: {}", e)))?;

    let body = aws_sdk_sesv2::types::Body::builder().html(html_body).build();

    let message = aws_sdk_sesv2::types::Message::builder()
        .subject(subject)
        .body(body)
        .build();

    let content = aws_sdk_sesv2::types::EmailContent::builder()
        .simple(message)
        .build();

    ses_client
        .send_email()
        .from_email_address(sender)
        .destination(destination)
        .content(content)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to send email: {:?}", e);
            AppError::InternalError("ელფოსტის გაგზავნა ვერ მოხერხდა".to_string())
        })?;

    Ok(())
}
