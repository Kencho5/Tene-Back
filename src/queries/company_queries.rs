use chrono::NaiveDate;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{
        Company, CompanyAddress, CompanyAddressRequest, CompanyContact, CompanyContactRequest,
        CompanyContract, CompanyContractRequest, CompanyQuery, CompanyReportQuery, CompanyRequest,
        CompanyStatusCounts, CompanySummary, CompanyTariff, CompanyTariffRequest,
        CompanyWasteTotal, ContractQuery, InvoiceTotals, WasteTotal, WasteType,
    },
};

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 100;

const SUMMARY_SQL: &str = r#"
    SELECT c.*,
        CASE
            WHEN NOT c.is_active
                 OR p.last_pickup_date IS NULL
                 OR p.last_pickup_date < CURRENT_DATE - 90 THEN 'inactive'
            WHEN p.last_pickup_date < CURRENT_DATE - 30 OR p.pickups_recent < 3 THEN 'less_active'
            ELSE 'active'
        END AS activity_status,
        p.last_pickup_date,
        p.pickups_recent,
        ct.start_date AS contract_start_date,
        ct.end_date AS contract_end_date,
        (ct.end_date - CURRENT_DATE) AS contract_days_left,
        inv.unpaid_count AS unpaid_invoices_count,
        COALESCE(inv.unpaid_total, 0) AS unpaid_invoices_total
    FROM companies c
    LEFT JOIN LATERAL (
        SELECT MAX(pickup_date) AS last_pickup_date,
               COUNT(*) FILTER (WHERE pickup_date >= CURRENT_DATE - 90) AS pickups_recent
        FROM company_pickups
        WHERE company_id = c.id
    ) p ON TRUE
    LEFT JOIN LATERAL (
        SELECT start_date, end_date
        FROM company_contracts
        WHERE company_id = c.id
        ORDER BY (CURRENT_DATE BETWEEN start_date AND end_date) DESC, end_date DESC
        LIMIT 1
    ) ct ON TRUE
    LEFT JOIN LATERAL (
        SELECT COUNT(*) AS unpaid_count, SUM(total) AS unpaid_total
        FROM company_invoices
        WHERE company_id = c.id AND paid_at IS NULL
    ) inv ON TRUE
"#;

const CONTRACT_SQL: &str = r#"
    SELECT ct.id, ct.company_id, c.name AS company_name, ct.contract_number,
        ct.start_date, ct.end_date, ct.terms, ct.file_uuid, ct.file_extension, ct.file_name,
        CASE
            WHEN ct.end_date < CURRENT_DATE THEN 'expired'
            WHEN ct.start_date > CURRENT_DATE THEN 'upcoming'
            WHEN ct.end_date <= CURRENT_DATE + 30 THEN 'expiring'
            ELSE 'active'
        END AS status,
        (ct.end_date - CURRENT_DATE) AS days_left,
        ct.created_at, ct.updated_at
    FROM company_contracts ct
    JOIN companies c ON c.id = ct.company_id
"#;

const TARIFF_SQL: &str = r#"
    SELECT t.id, t.company_id, t.waste_type, t.price_per_kg, t.fixed_fee, t.effective_from,
        t.note, u.name AS created_by_name, t.created_at
    FROM company_tariffs t
    LEFT JOIN users u ON u.id = t.created_by
"#;

pub fn map_unique_violation(err: sqlx::Error, message: &str) -> AppError {
    match &err {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => {
            AppError::Conflict(message.to_string())
        }
        _ => AppError::DatabaseError(err),
    }
}

pub async fn find_summary(pool: &PgPool, id: i32) -> Result<Option<CompanySummary>> {
    let sql = format!("SELECT * FROM ({SUMMARY_SQL}) s WHERE s.id = $1");
    let summary = sqlx::query_as::<_, CompanySummary>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(summary)
}

pub async fn find_company<'e>(ex: impl PgExecutor<'e>, id: i32) -> Result<Option<Company>> {
    let company = sqlx::query_as::<_, Company>("SELECT * FROM companies WHERE id = $1")
        .bind(id)
        .fetch_optional(ex)
        .await?;
    Ok(company)
}

pub async fn exists(pool: &PgPool, id: i32) -> Result<bool> {
    let found: Option<i32> = sqlx::query_scalar("SELECT id FROM companies WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(found.is_some())
}

pub async fn search_companies(
    pool: &PgPool,
    params: &CompanyQuery,
) -> Result<(Vec<CompanySummary>, i64, i64, i64)> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0).max(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(format!(
        "SELECT s.*, COUNT(*) OVER() AS total_count FROM ({SUMMARY_SQL}) s WHERE 1=1"
    ));

    if let Some(query) = params.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let pattern = format!("%{}%", query);
        qb.push(" AND (s.name ILIKE ");
        qb.push_bind(pattern.clone());
        qb.push(" OR s.identification_code ILIKE ");
        qb.push_bind(pattern);
        qb.push(")");
    }
    if let Some(status) = &params.activity_status {
        qb.push(" AND s.activity_status = ");
        qb.push_bind(*status);
    }
    if let Some(is_active) = params.is_active {
        qb.push(" AND s.is_active = ");
        qb.push_bind(is_active);
    }

    qb.push(" ORDER BY s.name ASC LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        summary: CompanySummary,
        total_count: i64,
    }

    let rows = qb.build_query_as::<Row>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let companies = rows.into_iter().map(|r| r.summary).collect();

    Ok((companies, total, limit, offset))
}

pub async fn all_summaries(pool: &PgPool, company_id: Option<i32>) -> Result<Vec<CompanySummary>> {
    let sql = format!(
        "SELECT * FROM ({SUMMARY_SQL}) s WHERE ($1::int IS NULL OR s.id = $1) ORDER BY s.name"
    );
    let rows = sqlx::query_as::<_, CompanySummary>(&sql)
        .bind(company_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn create_company(pool: &PgPool, req: &CompanyRequest) -> Result<i32> {
    sqlx::query_scalar(
        r#"
        INSERT INTO companies (
            name, identification_code, company_type, is_active, payment_terms_days,
            payment_terms, requires_confirmation, notes
        )
        VALUES ($1, $2, $3, COALESCE($4, TRUE), COALESCE($5, 15), $6, COALESCE($7, TRUE), $8)
        RETURNING id
        "#,
    )
    .bind(req.name.trim())
    .bind(req.identification_code.trim())
    .bind(req.company_type)
    .bind(req.is_active)
    .bind(req.payment_terms_days)
    .bind(&req.payment_terms)
    .bind(req.requires_confirmation)
    .bind(&req.notes)
    .fetch_one(pool)
    .await
    .map_err(|e| map_unique_violation(e, "ამ საიდენტიფიკაციო კოდით კომპანია უკვე არსებობს"))
}

pub async fn update_company(pool: &PgPool, id: i32, req: &CompanyRequest) -> Result<bool> {
    let updated: Option<i32> = sqlx::query_scalar(
        r#"
        UPDATE companies
        SET name                  = $1,
            identification_code   = $2,
            company_type          = $3,
            is_active             = COALESCE($4, is_active),
            payment_terms_days    = COALESCE($5, payment_terms_days),
            payment_terms         = $6,
            requires_confirmation = COALESCE($7, requires_confirmation),
            notes                 = $8,
            updated_at            = NOW()
        WHERE id = $9
        RETURNING id
        "#,
    )
    .bind(req.name.trim())
    .bind(req.identification_code.trim())
    .bind(req.company_type)
    .bind(req.is_active)
    .bind(req.payment_terms_days)
    .bind(&req.payment_terms)
    .bind(req.requires_confirmation)
    .bind(&req.notes)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| map_unique_violation(e, "ამ საიდენტიფიკაციო კოდით კომპანია უკვე არსებობს"))?;
    Ok(updated.is_some())
}

pub async fn count_pickups(pool: &PgPool, company_id: i32) -> Result<i64> {
    let count = sqlx::query_scalar("SELECT COUNT(*) FROM company_pickups WHERE company_id = $1")
        .bind(company_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn delete_company(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM companies WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn list_addresses(
    pool: &PgPool,
    company_id: Option<i32>,
) -> Result<Vec<CompanyAddress>> {
    let rows = sqlx::query_as::<_, CompanyAddress>(
        "SELECT * FROM company_addresses WHERE ($1::int IS NULL OR company_id = $1) ORDER BY company_id, id",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_address(
    pool: &PgPool,
    company_id: i32,
    address_id: i32,
) -> Result<Option<CompanyAddress>> {
    let row = sqlx::query_as::<_, CompanyAddress>(
        "SELECT * FROM company_addresses WHERE id = $1 AND company_id = $2",
    )
    .bind(address_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create_address(
    pool: &PgPool,
    company_id: i32,
    req: &CompanyAddressRequest,
) -> Result<CompanyAddress> {
    let row = sqlx::query_as::<_, CompanyAddress>(
        r#"
        INSERT INTO company_addresses (company_id, label, city, address)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(company_id)
    .bind(&req.label)
    .bind(&req.city)
    .bind(req.address.trim())
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_address(
    pool: &PgPool,
    company_id: i32,
    address_id: i32,
    req: &CompanyAddressRequest,
) -> Result<Option<CompanyAddress>> {
    let row = sqlx::query_as::<_, CompanyAddress>(
        r#"
        UPDATE company_addresses
        SET label = $1, city = $2, address = $3
        WHERE id = $4 AND company_id = $5
        RETURNING *
        "#,
    )
    .bind(&req.label)
    .bind(&req.city)
    .bind(req.address.trim())
    .bind(address_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_address(pool: &PgPool, company_id: i32, address_id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM company_addresses WHERE id = $1 AND company_id = $2")
        .bind(address_id)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn list_contacts(pool: &PgPool, company_id: Option<i32>) -> Result<Vec<CompanyContact>> {
    let rows = sqlx::query_as::<_, CompanyContact>(
        "SELECT * FROM company_contacts WHERE ($1::int IS NULL OR company_id = $1) ORDER BY company_id, id",
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn invoice_recipients<'e>(ex: impl PgExecutor<'e>, company_id: i32) -> Result<Vec<String>> {
    let rows = sqlx::query_scalar(
        r#"
        SELECT DISTINCT LOWER(TRIM(email))
        FROM company_contacts
        WHERE company_id = $1 AND receives_invoices AND email IS NOT NULL AND TRIM(email) <> ''
        "#,
    )
    .bind(company_id)
    .fetch_all(ex)
    .await?;
    Ok(rows)
}

pub async fn create_contact(
    pool: &PgPool,
    company_id: i32,
    req: &CompanyContactRequest,
) -> Result<CompanyContact> {
    let row = sqlx::query_as::<_, CompanyContact>(
        r#"
        INSERT INTO company_contacts (company_id, full_name, phone, email, receives_invoices)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(company_id)
    .bind(req.full_name.trim())
    .bind(&req.phone)
    .bind(&req.email)
    .bind(req.receives_invoices)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_contact(
    pool: &PgPool,
    company_id: i32,
    contact_id: i32,
    req: &CompanyContactRequest,
) -> Result<Option<CompanyContact>> {
    let row = sqlx::query_as::<_, CompanyContact>(
        r#"
        UPDATE company_contacts
        SET full_name = $1, phone = $2, email = $3, receives_invoices = $4
        WHERE id = $5 AND company_id = $6
        RETURNING *
        "#,
    )
    .bind(req.full_name.trim())
    .bind(&req.phone)
    .bind(&req.email)
    .bind(req.receives_invoices)
    .bind(contact_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_contact(pool: &PgPool, company_id: i32, contact_id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM company_contacts WHERE id = $1 AND company_id = $2")
        .bind(contact_id)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn list_contracts(pool: &PgPool, params: &ContractQuery) -> Result<Vec<CompanyContract>> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(format!(
        "SELECT * FROM ({CONTRACT_SQL}) x WHERE 1=1"
    ));

    if let Some(company_id) = params.company_id {
        qb.push(" AND x.company_id = ");
        qb.push_bind(company_id);
    }
    if let Some(status) = &params.status {
        qb.push(" AND x.status = ");
        qb.push_bind(*status);
    }
    if params.current_only {
        qb.push(
            " AND NOT EXISTS (SELECT 1 FROM company_contracts o \
              WHERE o.company_id = x.company_id AND o.end_date > x.end_date)",
        );
    }

    qb.push(" ORDER BY x.end_date ASC, x.id ASC");

    let rows = qb
        .build_query_as::<CompanyContract>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn find_contract(
    pool: &PgPool,
    company_id: i32,
    contract_id: i32,
) -> Result<Option<CompanyContract>> {
    let sql = format!("{CONTRACT_SQL} WHERE ct.id = $1 AND ct.company_id = $2");
    let row = sqlx::query_as::<_, CompanyContract>(&sql)
        .bind(contract_id)
        .bind(company_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create_contract(
    pool: &PgPool,
    company_id: i32,
    req: &CompanyContractRequest,
) -> Result<i32> {
    let id = sqlx::query_scalar(
        r#"
        INSERT INTO company_contracts (company_id, contract_number, start_date, end_date, terms)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(company_id)
    .bind(&req.contract_number)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(&req.terms)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn update_contract(
    pool: &PgPool,
    company_id: i32,
    contract_id: i32,
    req: &CompanyContractRequest,
) -> Result<bool> {
    let updated: Option<i32> = sqlx::query_scalar(
        r#"
        UPDATE company_contracts
        SET contract_number    = $1,
            start_date         = $2,
            expiry_notified_at = CASE WHEN end_date <> $3 THEN NULL ELSE expiry_notified_at END,
            end_date           = $3,
            terms              = $4,
            updated_at         = NOW()
        WHERE id = $5 AND company_id = $6
        RETURNING id
        "#,
    )
    .bind(&req.contract_number)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(&req.terms)
    .bind(contract_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(updated.is_some())
}

pub async fn delete_contract(pool: &PgPool, company_id: i32, contract_id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM company_contracts WHERE id = $1 AND company_id = $2")
        .bind(contract_id)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn set_contract_file(
    pool: &PgPool,
    contract_id: i32,
    file: Option<(Uuid, &str, &str)>,
) -> Result<()> {
    let (uuid, extension, name) = match file {
        Some((u, e, n)) => (Some(u), Some(e), Some(n)),
        None => (None, None, None),
    };
    sqlx::query(
        r#"
        UPDATE company_contracts
        SET file_uuid = $1, file_extension = $2, file_name = $3, updated_at = NOW()
        WHERE id = $4
        "#,
    )
    .bind(uuid)
    .bind(extension)
    .bind(name)
    .bind(contract_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn contracts_due_for_alert(pool: &PgPool, within_days: i32) -> Result<Vec<CompanyContract>> {
    let sql = format!(
        r#"
        SELECT x.* FROM ({CONTRACT_SQL}) x
        JOIN company_contracts raw ON raw.id = x.id
        WHERE raw.expiry_notified_at IS NULL
          AND x.end_date >= CURRENT_DATE
          AND x.end_date <= CURRENT_DATE + $1
          AND NOT EXISTS (
              SELECT 1 FROM company_contracts o
              WHERE o.company_id = x.company_id AND o.end_date > x.end_date
          )
        ORDER BY x.end_date ASC
        "#
    );
    let rows = sqlx::query_as::<_, CompanyContract>(&sql)
        .bind(within_days)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn mark_contracts_notified(pool: &PgPool, ids: &[i32]) -> Result<()> {
    sqlx::query("UPDATE company_contracts SET expiry_notified_at = NOW() WHERE id = ANY($1)")
        .bind(ids)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn tariff_history(pool: &PgPool, company_id: Option<i32>) -> Result<Vec<CompanyTariff>> {
    let sql = format!(
        "{TARIFF_SQL} WHERE ($1::int IS NULL OR t.company_id = $1) \
         ORDER BY t.company_id, t.effective_from DESC, t.waste_type"
    );
    let rows = sqlx::query_as::<_, CompanyTariff>(&sql)
        .bind(company_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn current_tariffs(pool: &PgPool, company_id: i32) -> Result<Vec<CompanyTariff>> {
    let sql = format!(
        r#"
        SELECT DISTINCT ON (x.waste_type) x.* FROM ({TARIFF_SQL}) x
        WHERE x.company_id = $1 AND x.effective_from <= CURRENT_DATE
        ORDER BY x.waste_type, x.effective_from DESC
        "#
    );
    let rows = sqlx::query_as::<_, CompanyTariff>(&sql)
        .bind(company_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn tariff_for_date<'e>(
    ex: impl PgExecutor<'e>,
    company_id: i32,
    waste_type: WasteType,
    date: NaiveDate,
) -> Result<Option<CompanyTariff>> {
    let sql = format!(
        "{TARIFF_SQL} WHERE t.company_id = $1 AND t.waste_type = $2 AND t.effective_from <= $3 \
         ORDER BY t.effective_from DESC LIMIT 1"
    );
    let row = sqlx::query_as::<_, CompanyTariff>(&sql)
        .bind(company_id)
        .bind(waste_type)
        .bind(date)
        .fetch_optional(ex)
        .await?;
    Ok(row)
}

pub async fn create_tariff(
    pool: &PgPool,
    company_id: i32,
    user_id: i32,
    req: &CompanyTariffRequest,
) -> Result<i32> {
    sqlx::query_scalar(
        r#"
        INSERT INTO company_tariffs (company_id, waste_type, price_per_kg, fixed_fee, effective_from, note, created_by)
        VALUES ($1, $2, $3, COALESCE($4, 0), $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(company_id)
    .bind(req.waste_type)
    .bind(req.price_per_kg)
    .bind(req.fixed_fee)
    .bind(req.effective_from)
    .bind(&req.note)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        map_unique_violation(e, "ამ ნარჩენის ტიპზე ამ თარიღიდან ტარიფი უკვე არსებობს")
    })
}

pub async fn delete_tariff(pool: &PgPool, company_id: i32, tariff_id: i32) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM company_tariffs t
        WHERE t.id = $1 AND t.company_id = $2
          AND NOT EXISTS (
              SELECT 1 FROM company_pickups p
              WHERE p.tariff_id = t.id AND p.status = 'confirmed'
          )
        "#,
    )
    .bind(tariff_id)
    .bind(company_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn status_counts(pool: &PgPool, company_id: Option<i32>) -> Result<CompanyStatusCounts> {
    let sql = format!(
        r#"
        SELECT
            COUNT(*) AS companies_total,
            COUNT(*) FILTER (WHERE s.activity_status = 'active') AS active,
            COUNT(*) FILTER (WHERE s.activity_status = 'less_active') AS less_active,
            COUNT(*) FILTER (WHERE s.activity_status = 'inactive') AS inactive,
            COUNT(*) FILTER (WHERE s.contract_end_date < CURRENT_DATE) AS contracts_expired,
            COUNT(*) FILTER (
                WHERE s.contract_end_date >= CURRENT_DATE
                  AND s.contract_end_date <= CURRENT_DATE + 30
            ) AS contracts_expiring
        FROM ({SUMMARY_SQL}) s
        WHERE ($1::int IS NULL OR s.id = $1)
        "#
    );
    let row = sqlx::query_as::<_, CompanyStatusCounts>(&sql)
        .bind(company_id)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn pickup_counts(pool: &PgPool, q: &CompanyReportQuery) -> Result<(i64, i64)> {
    let row: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COUNT(*) FILTER (WHERE status = 'pending')
        FROM company_pickups
        WHERE ($1::int IS NULL OR company_id = $1)
          AND ($2::date IS NULL OR pickup_date >= $2)
          AND ($3::date IS NULL OR pickup_date <= $3)
        "#,
    )
    .bind(q.company_id)
    .bind(q.from)
    .bind(q.to)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn waste_totals(pool: &PgPool, q: &CompanyReportQuery) -> Result<Vec<WasteTotal>> {
    let rows = sqlx::query_as::<_, WasteTotal>(
        r#"
        SELECT waste_type,
               COALESCE(SUM(quantity_kg), 0) AS quantity_kg,
               COALESCE(SUM(amount), 0) AS amount,
               COUNT(*) AS pickups_count
        FROM company_pickups
        WHERE ($1::int IS NULL OR company_id = $1)
          AND ($2::date IS NULL OR pickup_date >= $2)
          AND ($3::date IS NULL OR pickup_date <= $3)
        GROUP BY waste_type
        "#,
    )
    .bind(q.company_id)
    .bind(q.from)
    .bind(q.to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn per_company_totals(
    pool: &PgPool,
    q: &CompanyReportQuery,
) -> Result<Vec<CompanyWasteTotal>> {
    let rows = sqlx::query_as::<_, CompanyWasteTotal>(
        r#"
        SELECT c.id AS company_id,
               c.name AS company_name,
               COALESCE(SUM(p.quantity_kg) FILTER (WHERE p.waste_type = 'plastic'), 0) AS plastic_kg,
               COALESCE(SUM(p.quantity_kg) FILTER (WHERE p.waste_type = 'paper'), 0) AS paper_kg,
               COALESCE(SUM(p.quantity_kg) FILTER (WHERE p.waste_type = 'aluminum'), 0) AS aluminum_kg,
               COALESCE(SUM(p.quantity_kg), 0) AS total_kg,
               COALESCE(SUM(p.amount), 0) AS amount,
               COUNT(*) AS pickups_count
        FROM company_pickups p
        JOIN companies c ON c.id = p.company_id
        WHERE ($1::int IS NULL OR p.company_id = $1)
          AND ($2::date IS NULL OR p.pickup_date >= $2)
          AND ($3::date IS NULL OR p.pickup_date <= $3)
        GROUP BY c.id, c.name
        ORDER BY total_kg DESC, c.name
        "#,
    )
    .bind(q.company_id)
    .bind(q.from)
    .bind(q.to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn invoice_totals(pool: &PgPool, q: &CompanyReportQuery) -> Result<InvoiceTotals> {
    let row = sqlx::query_as::<_, InvoiceTotals>(
        r#"
        SELECT
            COUNT(*) AS invoices_count,
            COALESCE(SUM(total), 0) AS total_amount,
            COUNT(*) FILTER (WHERE paid_at IS NOT NULL) AS paid_count,
            COALESCE(SUM(total) FILTER (WHERE paid_at IS NOT NULL), 0) AS paid_amount,
            COUNT(*) FILTER (WHERE paid_at IS NULL) AS unpaid_count,
            COALESCE(SUM(total) FILTER (WHERE paid_at IS NULL), 0) AS unpaid_amount,
            COUNT(*) FILTER (WHERE paid_at IS NULL AND due_date < CURRENT_DATE) AS overdue_count,
            COALESCE(SUM(total) FILTER (WHERE paid_at IS NULL AND due_date < CURRENT_DATE), 0) AS overdue_amount
        FROM company_invoices
        WHERE ($1::int IS NULL OR company_id = $1)
          AND ($2::date IS NULL OR issue_date >= $2)
          AND ($3::date IS NULL OR issue_date <= $3)
        "#,
    )
    .bind(q.company_id)
    .bind(q.from)
    .bind(q.to)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
