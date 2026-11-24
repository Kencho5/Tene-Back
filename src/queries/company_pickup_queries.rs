use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgExecutor, PgPool};

use crate::{
    error::Result,
    models::{
        CompanyInvoice, CompanyPickup, InvoiceDelivery, InvoiceQuery, PickupQuery, PickupStatus,
        WasteType,
    },
};

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 200;

const PICKUP_SQL: &str = r#"
    SELECT p.id, p.company_id, c.name AS company_name, p.address_id, p.address_snapshot,
        p.pickup_date, p.waste_type, p.quantity_kg, p.tariff_id, p.unit_price, p.fixed_fee,
        p.amount, p.amount_overridden, p.status, p.notes,
        cu.name AS created_by_name, fu.name AS confirmed_by_name, p.confirmed_at,
        i.id AS invoice_id, i.invoice_number, p.created_at, p.updated_at
    FROM company_pickups p
    JOIN companies c ON c.id = p.company_id
    LEFT JOIN users cu ON cu.id = p.created_by
    LEFT JOIN users fu ON fu.id = p.confirmed_by
    LEFT JOIN company_invoices i ON i.pickup_id = p.id
"#;

const INVOICE_SQL: &str = r#"
    SELECT i.id, i.invoice_number, i.company_id, c.name AS company_name, i.pickup_id,
        p.pickup_date, p.waste_type, p.quantity_kg, i.issue_date, i.due_date, i.total,
        CASE
            WHEN i.paid_at IS NOT NULL THEN 'paid'
            WHEN i.due_date < CURRENT_DATE THEN 'overdue'
            WHEN i.last_sent_at IS NOT NULL THEN 'sent'
            ELSE 'unpaid'
        END AS status,
        i.paid_at, i.last_sent_at, i.created_at
    FROM company_invoices i
    JOIN companies c ON c.id = i.company_id
    JOIN company_pickups p ON p.id = i.pickup_id
"#;

pub struct NewPickup<'a> {
    pub company_id: i32,
    pub address_id: Option<i32>,
    pub address_snapshot: Option<&'a str>,
    pub pickup_date: NaiveDate,
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub tariff_id: Option<i32>,
    pub unit_price: Option<Decimal>,
    pub fixed_fee: Decimal,
    pub amount: Decimal,
    pub amount_overridden: bool,
    pub notes: Option<&'a str>,
}

#[derive(sqlx::FromRow)]
pub struct LockedPickup {
    pub id: i32,
    pub company_id: i32,
    pub pickup_date: NaiveDate,
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub amount: Decimal,
    pub amount_overridden: bool,
    pub tariff_id: Option<i32>,
    pub unit_price: Option<Decimal>,
    pub fixed_fee: Decimal,
    pub status: PickupStatus,
    pub address_snapshot: Option<String>,
}

pub async fn find_pickup<'e>(ex: impl PgExecutor<'e>, id: i32) -> Result<Option<CompanyPickup>> {
    let sql = format!("{PICKUP_SQL} WHERE p.id = $1");
    let row = sqlx::query_as::<_, CompanyPickup>(&sql)
        .bind(id)
        .fetch_optional(ex)
        .await?;
    Ok(row)
}

pub async fn search_pickups(
    pool: &PgPool,
    params: &PickupQuery,
    paginate: bool,
) -> Result<(Vec<CompanyPickup>, i64, Decimal, Decimal, i64, i64)> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(format!(
        "SELECT x.*, COUNT(*) OVER() AS total_count, \
         SUM(x.quantity_kg) OVER() AS total_quantity, SUM(x.amount) OVER() AS total_amount \
         FROM ({PICKUP_SQL}) x WHERE 1=1"
    ));

    if let Some(company_id) = params.company_id {
        qb.push(" AND x.company_id = ");
        qb.push_bind(company_id);
    }
    if let Some(waste_type) = params.waste_type {
        qb.push(" AND x.waste_type = ");
        qb.push_bind(waste_type);
    }
    if let Some(status) = params.status {
        qb.push(" AND x.status = ");
        qb.push_bind(status);
    }
    if let Some(from) = params.from {
        qb.push(" AND x.pickup_date >= ");
        qb.push_bind(from);
    }
    if let Some(to) = params.to {
        qb.push(" AND x.pickup_date <= ");
        qb.push_bind(to);
    }

    qb.push(" ORDER BY x.pickup_date DESC, x.id DESC");
    if paginate {
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        pickup: CompanyPickup,
        total_count: i64,
        total_quantity: Decimal,
        total_amount: Decimal,
    }

    let rows = qb.build_query_as::<Row>().fetch_all(pool).await?;
    let (total, total_quantity, total_amount) = rows
        .first()
        .map(|r| (r.total_count, r.total_quantity, r.total_amount))
        .unwrap_or((0, Decimal::ZERO, Decimal::ZERO));
    let pickups = rows.into_iter().map(|r| r.pickup).collect();

    Ok((pickups, total, total_quantity, total_amount, limit, offset))
}

pub async fn insert_pickup<'e>(
    ex: impl PgExecutor<'e>,
    p: &NewPickup<'_>,
    created_by: i32,
) -> Result<i32> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO company_pickups (
            company_id, address_id, address_snapshot, pickup_date, waste_type, quantity_kg,
            tariff_id, unit_price, fixed_fee, amount, amount_overridden, notes, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id
        "#,
    )
    .bind(p.company_id)
    .bind(p.address_id)
    .bind(p.address_snapshot)
    .bind(p.pickup_date)
    .bind(p.waste_type)
    .bind(p.quantity_kg)
    .bind(p.tariff_id)
    .bind(p.unit_price)
    .bind(p.fixed_fee)
    .bind(p.amount)
    .bind(p.amount_overridden)
    .bind(p.notes)
    .bind(created_by)
    .fetch_one(ex)
    .await?;
    Ok(id)
}

pub async fn update_pending_pickup(pool: &PgPool, id: i32, p: &NewPickup<'_>) -> Result<bool> {
    let updated: Option<i32> = sqlx::query_scalar(
        r#"
        UPDATE company_pickups
        SET company_id = $1, address_id = $2, address_snapshot = $3, pickup_date = $4,
            waste_type = $5, quantity_kg = $6, tariff_id = $7, unit_price = $8, fixed_fee = $9,
            amount = $10, amount_overridden = $11, notes = $12, updated_at = NOW()
        WHERE id = $13 AND status = 'pending'
        RETURNING id
        "#,
    )
    .bind(p.company_id)
    .bind(p.address_id)
    .bind(p.address_snapshot)
    .bind(p.pickup_date)
    .bind(p.waste_type)
    .bind(p.quantity_kg)
    .bind(p.tariff_id)
    .bind(p.unit_price)
    .bind(p.fixed_fee)
    .bind(p.amount)
    .bind(p.amount_overridden)
    .bind(p.notes)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(updated.is_some())
}

pub async fn delete_pending_pickup(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM company_pickups WHERE id = $1 AND status = 'pending'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn lock_pickup<'e>(ex: impl PgExecutor<'e>, id: i32) -> Result<Option<LockedPickup>> {
    let row = sqlx::query_as::<_, LockedPickup>(
        r#"
        SELECT id, company_id, pickup_date, waste_type, quantity_kg, amount, amount_overridden,
               tariff_id, unit_price, fixed_fee, status, address_snapshot
        FROM company_pickups
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(id)
    .fetch_optional(ex)
    .await?;
    Ok(row)
}

pub async fn mark_pickup_confirmed<'e>(
    ex: impl PgExecutor<'e>,
    p: &LockedPickup,
    confirmed_by: i32,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE company_pickups
        SET status = 'confirmed', confirmed_by = $1, confirmed_at = NOW(),
            tariff_id = $2, unit_price = $3, fixed_fee = $4, amount = $5, updated_at = NOW()
        WHERE id = $6
        "#,
    )
    .bind(confirmed_by)
    .bind(p.tariff_id)
    .bind(p.unit_price)
    .bind(p.fixed_fee)
    .bind(p.amount)
    .bind(p.id)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn insert_invoice<'e>(
    ex: impl PgExecutor<'e>,
    company_id: i32,
    pickup_id: i32,
    issue_date: NaiveDate,
    due_date: NaiveDate,
    total: Decimal,
) -> Result<(i32, String)> {
    let row = sqlx::query_as(
        r#"
        INSERT INTO company_invoices (company_id, pickup_id, issue_date, due_date, total, html)
        VALUES ($1, $2, $3, $4, $5, '')
        RETURNING id, invoice_number
        "#,
    )
    .bind(company_id)
    .bind(pickup_id)
    .bind(issue_date)
    .bind(due_date)
    .bind(total)
    .fetch_one(ex)
    .await?;
    Ok(row)
}

pub async fn set_invoice_html<'e>(ex: impl PgExecutor<'e>, id: i32, html: &str) -> Result<()> {
    sqlx::query("UPDATE company_invoices SET html = $1 WHERE id = $2")
        .bind(html)
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn find_invoice(pool: &PgPool, id: i32) -> Result<Option<CompanyInvoice>> {
    let sql = format!("{INVOICE_SQL} WHERE i.id = $1");
    let row = sqlx::query_as::<_, CompanyInvoice>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn invoice_html(pool: &PgPool, id: i32) -> Result<Option<String>> {
    let html = sqlx::query_scalar("SELECT html FROM company_invoices WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(html)
}

pub async fn search_invoices(
    pool: &PgPool,
    params: &InvoiceQuery,
    paginate: bool,
) -> Result<(Vec<CompanyInvoice>, i64, i64, i64)> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(format!(
        "SELECT x.*, COUNT(*) OVER() AS total_count FROM ({INVOICE_SQL}) x WHERE 1=1"
    ));

    if let Some(company_id) = params.company_id {
        qb.push(" AND x.company_id = ");
        qb.push_bind(company_id);
    }
    if let Some(status) = params.status {
        qb.push(" AND x.status = ");
        qb.push_bind(status);
    }
    if let Some(from) = params.from {
        qb.push(" AND x.issue_date >= ");
        qb.push_bind(from);
    }
    if let Some(to) = params.to {
        qb.push(" AND x.issue_date <= ");
        qb.push_bind(to);
    }

    qb.push(" ORDER BY x.issue_date DESC, x.id DESC");
    if paginate {
        qb.push(" LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        invoice: CompanyInvoice,
        total_count: i64,
    }

    let rows = qb.build_query_as::<Row>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let invoices = rows.into_iter().map(|r| r.invoice).collect();

    Ok((invoices, total, limit, offset))
}

pub async fn set_invoice_paid(pool: &PgPool, id: i32, paid: bool) -> Result<bool> {
    let updated: Option<i32> = sqlx::query_scalar(
        r#"
        UPDATE company_invoices
        SET paid_at = CASE WHEN $1 THEN COALESCE(paid_at, NOW()) ELSE NULL END,
            updated_at = NOW()
        WHERE id = $2
        RETURNING id
        "#,
    )
    .bind(paid)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(updated.is_some())
}

pub async fn record_delivery(
    pool: &PgPool,
    invoice_id: i32,
    recipients: &[String],
    error: Option<&str>,
    sent_by: Option<i32>,
) -> Result<InvoiceDelivery> {
    let mut tx = pool.begin().await?;

    let id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO company_invoice_deliveries (invoice_id, recipients, success, error, sent_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(invoice_id)
    .bind(recipients)
    .bind(error.is_none())
    .bind(error)
    .bind(sent_by)
    .fetch_one(&mut *tx)
    .await?;

    if error.is_none() {
        sqlx::query(
            "UPDATE company_invoices SET last_sent_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;
    }

    let delivery = sqlx::query_as::<_, InvoiceDelivery>(
        r#"
        SELECT d.id, d.invoice_id, d.recipients, d.success, d.error, u.name AS sent_by_name, d.sent_at
        FROM company_invoice_deliveries d
        LEFT JOIN users u ON u.id = d.sent_by
        WHERE d.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(delivery)
}

pub async fn list_deliveries(pool: &PgPool, invoice_id: i32) -> Result<Vec<InvoiceDelivery>> {
    let rows = sqlx::query_as::<_, InvoiceDelivery>(
        r#"
        SELECT d.id, d.invoice_id, d.recipients, d.success, d.error, u.name AS sent_by_name, d.sent_at
        FROM company_invoice_deliveries d
        LEFT JOIN users u ON u.id = d.sent_by
        WHERE d.invoice_id = $1
        ORDER BY d.sent_at DESC
        "#,
    )
    .bind(invoice_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
