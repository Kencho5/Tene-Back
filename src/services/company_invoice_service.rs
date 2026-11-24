use chrono::{Duration, FixedOffset, NaiveDate, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgExecutor;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{Company, InvoiceDelivery, PickupConfirmResponse, PickupStatus, WasteType},
    queries::{
        company_pickup_queries::{self, LockedPickup},
        company_queries,
    },
    services::email_service::{format_money, html_escape, send_html_email},
};

const NO_RECIPIENTS: &str = "ინვოისის მიმღები ელფოსტა მონიშნული არ არის";

pub struct PickupPricing {
    pub tariff_id: Option<i32>,
    pub unit_price: Option<Decimal>,
    pub fixed_fee: Decimal,
    pub amount: Decimal,
    pub amount_overridden: bool,
}

fn round_money(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

pub fn today() -> NaiveDate {
    let tbilisi = FixedOffset::east_opt(4 * 3600).unwrap();
    Utc::now().with_timezone(&tbilisi).date_naive()
}

pub async fn price_pickup<'e>(
    ex: impl PgExecutor<'e>,
    company_id: i32,
    waste_type: WasteType,
    pickup_date: NaiveDate,
    quantity_kg: Decimal,
    manual_amount: Option<Decimal>,
) -> Result<PickupPricing> {
    let tariff =
        company_queries::tariff_for_date(ex, company_id, waste_type, pickup_date).await?;

    if let Some(amount) = manual_amount {
        if amount < Decimal::ZERO {
            return Err(AppError::BadRequest(
                "ღირებულება უარყოფითი ვერ იქნება".to_string(),
            ));
        }
        return Ok(PickupPricing {
            tariff_id: tariff.as_ref().map(|t| t.id),
            unit_price: tariff.as_ref().map(|t| t.price_per_kg),
            fixed_fee: Decimal::ZERO,
            amount: round_money(amount),
            amount_overridden: true,
        });
    }

    let tariff = tariff.ok_or_else(|| {
        AppError::BadRequest(format!(
            "{} — ამ თარიღისთვის ტარიფი განსაზღვრული არ არის. დაამატეთ ტარიფი ან მიუთითეთ ღირებულება",
            waste_type.label()
        ))
    })?;

    Ok(PickupPricing {
        tariff_id: Some(tariff.id),
        unit_price: Some(tariff.price_per_kg),
        fixed_fee: tariff.fixed_fee,
        amount: round_money(tariff.fixed_fee + quantity_kg * tariff.price_per_kg),
        amount_overridden: false,
    })
}

pub async fn confirm_pickup(
    state: &AppState,
    pickup_id: i32,
    confirmed_by: i32,
) -> Result<PickupConfirmResponse> {
    let mut tx = state.db.begin().await?;

    let mut pickup = company_pickup_queries::lock_pickup(&mut *tx, pickup_id)
        .await?
        .ok_or_else(|| AppError::NotFound("გატანა ვერ მოიძებნა".to_string()))?;

    if pickup.status == PickupStatus::Confirmed {
        return Err(AppError::Conflict("გატანა უკვე დადასტურებულია".to_string()));
    }

    if !pickup.amount_overridden {
        let pricing = price_pickup(
            &mut *tx,
            pickup.company_id,
            pickup.waste_type,
            pickup.pickup_date,
            pickup.quantity_kg,
            None,
        )
        .await?;
        pickup.tariff_id = pricing.tariff_id;
        pickup.unit_price = pricing.unit_price;
        pickup.fixed_fee = pricing.fixed_fee;
        pickup.amount = pricing.amount;
    }

    company_pickup_queries::mark_pickup_confirmed(&mut *tx, &pickup, confirmed_by).await?;

    let company = company_queries::find_company(&mut *tx, pickup.company_id)
        .await?
        .ok_or_else(|| AppError::NotFound("კომპანია ვერ მოიძებნა".to_string()))?;

    let issue_date = today();
    let due_date = issue_date + Duration::days(company.payment_terms_days as i64);

    let (invoice_id, invoice_number) = company_pickup_queries::insert_invoice(
        &mut *tx,
        company.id,
        pickup.id,
        issue_date,
        due_date,
        pickup.amount,
    )
    .await?;

    let html = render_invoice(&company, &pickup, &invoice_number, issue_date, due_date);
    company_pickup_queries::set_invoice_html(&mut *tx, invoice_id, &html).await?;

    tx.commit().await?;

    let delivery = match deliver_invoice(state, invoice_id, Some(confirmed_by)).await {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::error!("Invoice {} delivery failed: {}", invoice_number, e);
            None
        }
    };

    let pickup = company_pickup_queries::find_pickup(&state.db, pickup_id)
        .await?
        .ok_or_else(|| AppError::NotFound("გატანა ვერ მოიძებნა".to_string()))?;
    let invoice = company_pickup_queries::find_invoice(&state.db, invoice_id).await?;

    Ok(PickupConfirmResponse {
        pickup,
        invoice,
        delivery,
    })
}

pub async fn deliver_invoice(
    state: &AppState,
    invoice_id: i32,
    sent_by: Option<i32>,
) -> Result<InvoiceDelivery> {
    let invoice = company_pickup_queries::find_invoice(&state.db, invoice_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ინვოისი ვერ მოიძებნა".to_string()))?;
    let html = company_pickup_queries::invoice_html(&state.db, invoice_id)
        .await?
        .unwrap_or_default();
    let recipients = company_queries::invoice_recipients(&state.db, invoice.company_id).await?;

    if recipients.is_empty() {
        return company_pickup_queries::record_delivery(
            &state.db,
            invoice_id,
            &recipients,
            Some(NO_RECIPIENTS),
            sent_by,
        )
        .await;
    }

    let to: Vec<&str> = recipients.iter().map(String::as_str).collect();
    let subject = format!("ინვოისი {} · Tene", invoice.invoice_number);
    let error = send_html_email(&state.ses_client, &to, &subject, &html)
        .await
        .err()
        .map(|e| e.to_string());

    company_pickup_queries::record_delivery(
        &state.db,
        invoice_id,
        &recipients,
        error.as_deref(),
        sent_by,
    )
    .await
}

fn format_date(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

fn format_kg(quantity: Decimal) -> String {
    format!("{} კგ", quantity.normalize())
}

fn render_invoice(
    company: &Company,
    pickup: &LockedPickup,
    invoice_number: &str,
    issue_date: NaiveDate,
    due_date: NaiveDate,
) -> String {
    let service = format!("{} ნარჩენის გატანა", pickup.waste_type.label());
    let pickup_date = format_date(pickup.pickup_date);

    let mut rows = String::new();
    let main_row = |unit: String, line: Decimal| {
        format!(
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{} ₾</td></tr>",
            html_escape(&service),
            pickup_date,
            format_kg(pickup.quantity_kg),
            unit,
            format_money(line),
        )
    };

    match (pickup.amount_overridden, pickup.unit_price) {
        (false, Some(unit_price)) => {
            rows.push_str(&main_row(
                format!("{} ₾/კგ", format_money(unit_price)),
                round_money(pickup.quantity_kg * unit_price),
            ));
            if pickup.fixed_fee > Decimal::ZERO {
                rows.push_str(&format!(
                    "<tr><td>მომსახურების საფასური</td><td>{}</td><td class=\"num\">1</td><td class=\"num\">{} ₾</td><td class=\"num\">{} ₾</td></tr>",
                    pickup_date,
                    format_money(pickup.fixed_fee),
                    format_money(pickup.fixed_fee),
                ));
            }
        }
        _ => rows.push_str(&main_row("—".to_string(), pickup.amount)),
    }

    let address_block = pickup
        .address_snapshot
        .as_deref()
        .filter(|a| !a.trim().is_empty())
        .map(|a| format!("<div class=\"sub\">{}</div>", html_escape(a)))
        .unwrap_or_default();

    let payment_terms_block = company
        .payment_terms
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(|t| {
            format!(
                "<div class=\"note\"><strong>გადახდის პირობები:</strong> {}</div>",
                html_escape(t)
            )
        })
        .unwrap_or_default();

    include_str!("../utils/company_invoice.html")
        .replace("{{invoice_number}}", &html_escape(invoice_number))
        .replace("{{company_name}}", &html_escape(&company.name))
        .replace("{{company_type}}", company.company_type.label())
        .replace(
            "{{identification_code}}",
            &html_escape(&company.identification_code),
        )
        .replace("{{address_block}}", &address_block)
        .replace("{{issue_date}}", &format_date(issue_date))
        .replace("{{due_date}}", &format_date(due_date))
        .replace("{{items_rows}}", &rows)
        .replace("{{total}}", &format_money(pickup.amount))
        .replace("{{payment_terms_block}}", &payment_terms_block)
}
