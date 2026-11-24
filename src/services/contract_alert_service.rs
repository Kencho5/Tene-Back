use std::time::Duration;

use aws_sdk_sesv2::Client as SesClient;
use sqlx::PgPool;

use crate::{
    error::Result,
    queries::company_queries,
    services::email_service::{html_escape, send_html_email},
};

const ALERT_WINDOW_DAYS: i32 = 30;
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

pub fn spawn(db: PgPool, ses_client: SesClient) {
    let Some(recipient) = std::env::var("COMPANY_ALERT_EMAIL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        tracing::info!("COMPANY_ALERT_EMAIL not set, contract expiry alerts disabled");
        return;
    };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(CHECK_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = run(&db, &ses_client, &recipient).await {
                tracing::error!("Contract expiry alert failed: {}", e);
            }
        }
    });
}

async fn run(db: &PgPool, ses_client: &SesClient, recipient: &str) -> Result<()> {
    let contracts = company_queries::contracts_due_for_alert(db, ALERT_WINDOW_DAYS).await?;
    if contracts.is_empty() {
        return Ok(());
    }

    let rows: String = contracts
        .iter()
        .map(|c| {
            format!(
                "<tr><td style=\"padding:8px;border-bottom:1px solid #eee\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #eee\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #eee\">{}</td>\
                 <td style=\"padding:8px;border-bottom:1px solid #eee;text-align:right\">{}</td></tr>",
                html_escape(&c.company_name),
                html_escape(c.contract_number.as_deref().unwrap_or("—")),
                c.end_date.format("%d.%m.%Y"),
                c.days_left,
            )
        })
        .collect();

    let html = format!(
        "<div style=\"font-family:'Noto Sans Georgian',sans-serif;color:#212121\">\
         <h2 style=\"font-size:18px\">ხელშეკრულებები, რომელთა ვადა იწურება</h2>\
         <table style=\"border-collapse:collapse;width:100%;font-size:14px\">\
         <thead><tr>\
         <th style=\"text-align:left;padding:8px\">კომპანია</th>\
         <th style=\"text-align:left;padding:8px\">ნომერი</th>\
         <th style=\"text-align:left;padding:8px\">დასრულება</th>\
         <th style=\"text-align:right;padding:8px\">დარჩენილი დღე</th>\
         </tr></thead><tbody>{rows}</tbody></table></div>"
    );

    send_html_email(
        ses_client,
        &[recipient],
        "ხელშეკრულებების ვადის ამოწურვის შეტყობინება",
        &html,
    )
    .await?;

    let ids: Vec<i32> = contracts.iter().map(|c| c.id).collect();
    company_queries::mark_contracts_notified(db, &ids).await
}
