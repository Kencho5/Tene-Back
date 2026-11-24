use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use http::StatusCode;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use rust_xlsxwriter::{Format, Workbook};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        CompanyAddress, CompanyAddressRequest, CompanyContact, CompanyContactRequest,
        CompanyContract, CompanyContractRequest, CompanyContractResponse, CompanyDetail,
        CompanyExportQuery, CompanyQuery, CompanyReport, CompanyReportQuery, CompanyRequest,
        CompanySearchResponse, CompanySummary, CompanyTariffRequest, ContractFileUploadRequest,
        ContractFileUploadResponse, ContractQuery, ExportDataset, InvoiceQuery, PickupQuery,
        WasteTotal, WasteType,
    },
    queries::{company_pickup_queries, company_queries},
    services::image_url_service::{delete_single_object, get_object_url, put_object_url},
    utils::jwt::Claims,
};

const FILE_URL_TTL_SECS: u64 = 900;

fn env_prefix(state: &AppState) -> &'static str {
    match state.environment {
        crate::config::Environment::Staging => "companies-staging",
        crate::config::Environment::Main => "companies-main",
    }
}

fn contract_file_key(state: &AppState, contract: &CompanyContract) -> Option<String> {
    let uuid = contract.file_uuid?;
    let ext = contract.file_extension.as_deref()?;
    Some(format!(
        "{}/{}/contracts/{}.{}",
        env_prefix(state),
        contract.company_id,
        uuid,
        ext
    ))
}

fn contract_file_ext(content_type: &str) -> Option<&'static str> {
    match content_type {
        "application/pdf" => Some("pdf"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "application/msword" => Some("doc"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        _ => None,
    }
}

async fn contract_response(state: &AppState, contract: CompanyContract) -> CompanyContractResponse {
    let file_url = match contract_file_key(state, &contract) {
        Some(key) => get_object_url(
            &state.s3_client,
            &state.s3_bucket,
            &key,
            contract.file_name.as_deref(),
            FILE_URL_TTL_SECS,
        )
        .await
        .map_err(|e| tracing::error!("Contract file URL presign failed: {}", e))
        .ok(),
        None => None,
    };
    CompanyContractResponse { contract, file_url }
}

async fn remove_contract_file(state: &AppState, contract: &CompanyContract) {
    if let Some(key) = contract_file_key(state, contract) {
        if let Err(e) = delete_single_object(&state.s3_client, &state.s3_bucket, &key).await {
            tracing::error!("Contract file delete failed for {}: {}", key, e);
        }
    }
}

fn is_blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|v| v.trim().is_empty())
}

fn company_not_found(id: i32) -> AppError {
    AppError::NotFound(format!("კომპანია id-ით {} ვერ მოიძებნა", id))
}

async fn ensure_company(state: &AppState, id: i32) -> Result<()> {
    if company_queries::exists(&state.db, id).await? {
        Ok(())
    } else {
        Err(company_not_found(id))
    }
}

fn validate_company(req: &CompanyRequest) -> Result<()> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("კომპანიის დასახელება აუცილებელია".to_string()));
    }
    if req.identification_code.trim().is_empty() {
        return Err(AppError::BadRequest("საიდენტიფიკაციო კოდი აუცილებელია".to_string()));
    }
    if req.payment_terms_days.is_some_and(|d| d < 0) {
        return Err(AppError::BadRequest(
            "გადახდის ვადა უარყოფითი ვერ იქნება".to_string(),
        ));
    }
    Ok(())
}

fn validate_contact(req: &CompanyContactRequest) -> Result<()> {
    if req.full_name.trim().is_empty() {
        return Err(AppError::BadRequest("სახელი და გვარი აუცილებელია".to_string()));
    }
    if let Some(email) = req.email.as_deref().filter(|e| !e.trim().is_empty()) {
        let email = email.trim();
        if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
            return Err(AppError::BadRequest("ელფოსტა არასწორია".to_string()));
        }
    }
    if req.receives_invoices && is_blank(&req.email) {
        return Err(AppError::BadRequest(
            "ინვოისის მიმღებისთვის ელფოსტა აუცილებელია".to_string(),
        ));
    }
    Ok(())
}

fn validate_contract(req: &CompanyContractRequest) -> Result<()> {
    if req.end_date < req.start_date {
        return Err(AppError::BadRequest(
            "დასრულების თარიღი დაწყების თარიღზე ადრე ვერ იქნება".to_string(),
        ));
    }
    Ok(())
}

async fn load_detail(state: &AppState, summary: CompanySummary) -> Result<CompanyDetail> {
    let id = summary.company.id;
    let addresses = company_queries::list_addresses(&state.db, Some(id)).await?;
    let contacts = company_queries::list_contacts(&state.db, Some(id)).await?;
    let contracts = company_queries::list_contracts(
        &state.db,
        &ContractQuery {
            company_id: Some(id),
            status: None,
            current_only: false,
        },
    )
    .await?;
    let current_tariffs = company_queries::current_tariffs(&state.db, id).await?;
    let tariff_history = company_queries::tariff_history(&state.db, Some(id)).await?;

    let mut contract_responses = Vec::with_capacity(contracts.len());
    for contract in contracts.into_iter().rev() {
        contract_responses.push(contract_response(state, contract).await);
    }

    Ok(CompanyDetail {
        summary,
        addresses,
        contacts,
        contracts: contract_responses,
        current_tariffs,
        tariff_history,
    })
}

async fn detail_by_id(state: &AppState, id: i32) -> Result<CompanyDetail> {
    let summary = company_queries::find_summary(&state.db, id)
        .await?
        .ok_or_else(|| company_not_found(id))?;
    load_detail(state, summary).await
}

pub async fn search_companies(
    State(state): State<AppState>,
    Query(params): Query<CompanyQuery>,
) -> Result<Json<CompanySearchResponse>> {
    let (companies, total, limit, offset) =
        company_queries::search_companies(&state.db, &params).await?;
    Ok(Json(CompanySearchResponse {
        companies,
        total,
        limit,
        offset,
    }))
}

pub async fn get_company(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<CompanyDetail>> {
    Ok(Json(detail_by_id(&state, id).await?))
}

pub async fn create_company(
    State(state): State<AppState>,
    Json(payload): Json<CompanyRequest>,
) -> Result<(StatusCode, Json<CompanyDetail>)> {
    validate_company(&payload)?;
    let id = company_queries::create_company(&state.db, &payload).await?;
    Ok((StatusCode::CREATED, Json(detail_by_id(&state, id).await?)))
}

pub async fn update_company(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyRequest>,
) -> Result<Json<CompanyDetail>> {
    validate_company(&payload)?;
    if !company_queries::update_company(&state.db, id, &payload).await? {
        return Err(company_not_found(id));
    }
    Ok(Json(detail_by_id(&state, id).await?))
}

pub async fn delete_company(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    ensure_company(&state, id).await?;
    if company_queries::count_pickups(&state.db, id).await? > 0 {
        return Err(AppError::Conflict(
            "კომპანიას აქვს გატანების ისტორია და ვერ წაიშლება. სანაცვლოდ გახადეთ არააქტიური"
                .to_string(),
        ));
    }

    let contracts = company_queries::list_contracts(
        &state.db,
        &ContractQuery {
            company_id: Some(id),
            status: None,
            current_only: false,
        },
    )
    .await?;
    for contract in &contracts {
        remove_contract_file(&state, contract).await;
    }

    company_queries::delete_company(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_address(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyAddressRequest>,
) -> Result<(StatusCode, Json<CompanyAddress>)> {
    if payload.address.trim().is_empty() {
        return Err(AppError::BadRequest("მისამართი აუცილებელია".to_string()));
    }
    ensure_company(&state, id).await?;
    let address = company_queries::create_address(&state.db, id, &payload).await?;
    Ok((StatusCode::CREATED, Json(address)))
}

pub async fn update_address(
    State(state): State<AppState>,
    Path((id, address_id)): Path<(i32, i32)>,
    Json(payload): Json<CompanyAddressRequest>,
) -> Result<Json<CompanyAddress>> {
    if payload.address.trim().is_empty() {
        return Err(AppError::BadRequest("მისამართი აუცილებელია".to_string()));
    }
    company_queries::update_address(&state.db, id, address_id, &payload)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("მისამართი ვერ მოიძებნა".to_string()))
}

pub async fn delete_address(
    State(state): State<AppState>,
    Path((id, address_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    if company_queries::delete_address(&state.db, id, address_id).await? == 0 {
        return Err(AppError::NotFound("მისამართი ვერ მოიძებნა".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_contact(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyContactRequest>,
) -> Result<(StatusCode, Json<CompanyContact>)> {
    validate_contact(&payload)?;
    ensure_company(&state, id).await?;
    let contact = company_queries::create_contact(&state.db, id, &payload).await?;
    Ok((StatusCode::CREATED, Json(contact)))
}

pub async fn update_contact(
    State(state): State<AppState>,
    Path((id, contact_id)): Path<(i32, i32)>,
    Json(payload): Json<CompanyContactRequest>,
) -> Result<Json<CompanyContact>> {
    validate_contact(&payload)?;
    company_queries::update_contact(&state.db, id, contact_id, &payload)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::NotFound("საკონტაქტო პირი ვერ მოიძებნა".to_string()))
}

pub async fn delete_contact(
    State(state): State<AppState>,
    Path((id, contact_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    if company_queries::delete_contact(&state.db, id, contact_id).await? == 0 {
        return Err(AppError::NotFound("საკონტაქტო პირი ვერ მოიძებნა".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_contracts(
    State(state): State<AppState>,
    Query(params): Query<ContractQuery>,
) -> Result<Json<Vec<CompanyContractResponse>>> {
    let contracts = company_queries::list_contracts(&state.db, &params).await?;
    let mut out = Vec::with_capacity(contracts.len());
    for contract in contracts {
        out.push(contract_response(&state, contract).await);
    }
    Ok(Json(out))
}

async fn find_contract(state: &AppState, id: i32, contract_id: i32) -> Result<CompanyContract> {
    company_queries::find_contract(&state.db, id, contract_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ხელშეკრულება ვერ მოიძებნა".to_string()))
}

pub async fn create_contract(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyContractRequest>,
) -> Result<(StatusCode, Json<CompanyContractResponse>)> {
    validate_contract(&payload)?;
    ensure_company(&state, id).await?;
    let contract_id = company_queries::create_contract(&state.db, id, &payload).await?;
    let contract = find_contract(&state, id, contract_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(contract_response(&state, contract).await),
    ))
}

pub async fn update_contract(
    State(state): State<AppState>,
    Path((id, contract_id)): Path<(i32, i32)>,
    Json(payload): Json<CompanyContractRequest>,
) -> Result<Json<CompanyContractResponse>> {
    validate_contract(&payload)?;
    if !company_queries::update_contract(&state.db, id, contract_id, &payload).await? {
        return Err(AppError::NotFound("ხელშეკრულება ვერ მოიძებნა".to_string()));
    }
    let contract = find_contract(&state, id, contract_id).await?;
    Ok(Json(contract_response(&state, contract).await))
}

pub async fn delete_contract(
    State(state): State<AppState>,
    Path((id, contract_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    let contract = find_contract(&state, id, contract_id).await?;
    company_queries::delete_contract(&state.db, id, contract_id).await?;
    remove_contract_file(&state, &contract).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_contract_file_url(
    State(state): State<AppState>,
    Path((id, contract_id)): Path<(i32, i32)>,
    Json(payload): Json<ContractFileUploadRequest>,
) -> Result<Json<ContractFileUploadResponse>> {
    let extension = contract_file_ext(&payload.content_type).ok_or_else(|| {
        AppError::BadRequest("დასაშვებია მხოლოდ PDF, Word ან სურათის ფაილი".to_string())
    })?;
    let file_name = payload.file_name.trim();
    let file_name = if file_name.is_empty() {
        format!("contract.{}", extension)
    } else {
        file_name.chars().take(255).collect()
    };

    let contract = find_contract(&state, id, contract_id).await?;
    let file_uuid = Uuid::new_v4();
    let key = format!(
        "{}/{}/contracts/{}.{}",
        env_prefix(&state),
        id,
        file_uuid,
        extension
    );

    let upload_url = put_object_url(
        &state.s3_client,
        &state.s3_bucket,
        &key,
        &payload.content_type,
        "private, max-age=0",
        FILE_URL_TTL_SECS,
    )
    .await
    .map_err(|e| {
        AppError::InternalError(format!(
            "წინასწარ ხელმოწერილი URL-ის გენერაცია ვერ მოხერხდა: {}",
            e
        ))
    })?;

    company_queries::set_contract_file(
        &state.db,
        contract_id,
        Some((file_uuid, extension, &file_name)),
    )
    .await?;
    remove_contract_file(&state, &contract).await;

    Ok(Json(ContractFileUploadResponse { upload_url }))
}

pub async fn delete_contract_file(
    State(state): State<AppState>,
    Path((id, contract_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    let contract = find_contract(&state, id, contract_id).await?;
    company_queries::set_contract_file(&state.db, contract_id, None).await?;
    remove_contract_file(&state, &contract).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_tariff(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyTariffRequest>,
) -> Result<(StatusCode, Json<CompanyDetail>)> {
    if payload.price_per_kg < Decimal::ZERO || payload.fixed_fee.is_some_and(|f| f < Decimal::ZERO)
    {
        return Err(AppError::BadRequest("ტარიფი უარყოფითი ვერ იქნება".to_string()));
    }
    ensure_company(&state, id).await?;
    company_queries::create_tariff(&state.db, id, claims.user_id, &payload).await?;
    Ok((StatusCode::CREATED, Json(detail_by_id(&state, id).await?)))
}

pub async fn delete_tariff(
    State(state): State<AppState>,
    Path((id, tariff_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    if company_queries::delete_tariff(&state.db, id, tariff_id).await? == 0 {
        return Err(AppError::Conflict(
            "ტარიფი ვერ მოიძებნა ან უკვე გამოყენებულია დადასტურებულ გატანებში".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_report(
    State(state): State<AppState>,
    Query(params): Query<CompanyReportQuery>,
) -> Result<Json<CompanyReport>> {
    let status_counts = company_queries::status_counts(&state.db, params.company_id).await?;
    let (pickups_count, pickups_pending) =
        company_queries::pickup_counts(&state.db, &params).await?;
    let mut totals = company_queries::waste_totals(&state.db, &params).await?;
    let per_company = company_queries::per_company_totals(&state.db, &params).await?;
    let invoices = company_queries::invoice_totals(&state.db, &params).await?;

    let waste_totals = WasteType::ALL
        .iter()
        .map(|wt| {
            totals
                .iter()
                .position(|t| t.waste_type == *wt)
                .map(|i| totals.swap_remove(i))
                .unwrap_or(WasteTotal {
                    waste_type: *wt,
                    quantity_kg: Decimal::ZERO,
                    amount: Decimal::ZERO,
                    pickups_count: 0,
                })
        })
        .collect();

    Ok(Json(CompanyReport {
        status_counts,
        pickups_count,
        pickups_pending,
        waste_totals,
        per_company,
        invoices,
    }))
}

enum Cell {
    Text(String),
    Number(f64),
}

impl From<String> for Cell {
    fn from(v: String) -> Self {
        Cell::Text(v)
    }
}

impl From<&str> for Cell {
    fn from(v: &str) -> Self {
        Cell::Text(v.to_string())
    }
}

impl From<Option<String>> for Cell {
    fn from(v: Option<String>) -> Self {
        Cell::Text(v.unwrap_or_default())
    }
}

impl From<Decimal> for Cell {
    fn from(v: Decimal) -> Self {
        Cell::Number(v.to_f64().unwrap_or(0.0))
    }
}

impl From<i64> for Cell {
    fn from(v: i64) -> Self {
        Cell::Number(v as f64)
    }
}

impl From<i32> for Cell {
    fn from(v: i32) -> Self {
        Cell::Number(v as f64)
    }
}

fn opt_date<T: ToString>(v: Option<T>) -> Cell {
    Cell::Text(v.map(|d| d.to_string()).unwrap_or_default())
}

fn yes_no(v: bool) -> Cell {
    Cell::Text(if v { "კი" } else { "არა" }.to_string())
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::InternalError(format!("excel-ის გენერაცია ვერ მოხერხდა: {}", e))
}

fn write_sheet(
    workbook: &mut Workbook,
    name: &str,
    headers: &[&str],
    rows: Vec<Vec<Cell>>,
) -> Result<()> {
    let bold = Format::new().set_bold();
    let sheet = workbook.add_worksheet();
    sheet.set_name(name).map_err(xlsx_err)?;

    for (col, title) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *title, &bold)
            .map_err(xlsx_err)?;
        sheet.set_column_width(col as u16, 18).map_err(xlsx_err)?;
    }

    for (i, row) in rows.into_iter().enumerate() {
        let r = (i + 1) as u32;
        for (col, cell) in row.into_iter().enumerate() {
            match cell {
                Cell::Text(v) => sheet.write_string(r, col as u16, v),
                Cell::Number(v) => sheet.write_number(r, col as u16, v),
            }
            .map_err(xlsx_err)?;
        }
    }
    Ok(())
}

pub async fn export_companies(
    State(state): State<AppState>,
    Query(params): Query<CompanyExportQuery>,
) -> Result<Response> {
    let wants = |d: ExportDataset| params.dataset.is_none_or(|selected| selected == d);
    let report_query = CompanyReportQuery {
        company_id: params.company_id,
        from: params.from,
        to: params.to,
    };

    let summaries = company_queries::all_summaries(&state.db, params.company_id).await?;
    let names: HashMap<i32, String> = summaries
        .iter()
        .map(|s| (s.company.id, s.company.name.clone()))
        .collect();
    let name_of = |id: i32| names.get(&id).cloned().unwrap_or_default();

    let mut workbook = Workbook::new();

    if wants(ExportDataset::Companies) {
        let rows = summaries
            .iter()
            .map(|s| {
                let c = &s.company;
                vec![
                    c.id.into(),
                    c.name.as_str().into(),
                    c.identification_code.as_str().into(),
                    c.company_type.label().into(),
                    (if c.is_active { "აქტიური" } else { "არააქტიური" }).into(),
                    s.activity_status.label().into(),
                    c.payment_terms_days.into(),
                    c.payment_terms.clone().into(),
                    opt_date(s.contract_start_date),
                    opt_date(s.contract_end_date),
                    s.contract_days_left
                        .map(Cell::from)
                        .unwrap_or_else(|| "".into()),
                    opt_date(s.last_pickup_date),
                    s.unpaid_invoices_count.into(),
                    s.unpaid_invoices_total.into(),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "კომპანიები",
            &[
                "ID",
                "დასახელება",
                "საიდენტიფიკაციო კოდი",
                "ტიპი",
                "სტატუსი",
                "აქტივობა",
                "გადახდის ვადა (დღე)",
                "გადახდის პირობები",
                "ხელშეკრულების დაწყება",
                "ხელშეკრულების დასრულება",
                "დარჩენილი დღე",
                "ბოლო გატანა",
                "გადაუხდელი ინვოისები",
                "გადაუხდელი თანხა (₾)",
            ],
            rows,
        )?;
    }

    if wants(ExportDataset::Addresses) {
        let rows = company_queries::list_addresses(&state.db, params.company_id)
            .await?
            .into_iter()
            .map(|a| {
                vec![
                    name_of(a.company_id).into(),
                    a.label.into(),
                    a.city.into(),
                    a.address.into(),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "მისამართები",
            &["კომპანია", "დასახელება", "ქალაქი", "მისამართი"],
            rows,
        )?;
    }

    if wants(ExportDataset::Contacts) {
        let rows = company_queries::list_contacts(&state.db, params.company_id)
            .await?
            .into_iter()
            .map(|c| {
                vec![
                    name_of(c.company_id).into(),
                    c.full_name.into(),
                    c.phone.into(),
                    c.email.into(),
                    yes_no(c.receives_invoices),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "საკონტაქტო პირები",
            &["კომპანია", "სახელი და გვარი", "ტელეფონი", "ელფოსტა", "იღებს ინვოისებს"],
            rows,
        )?;
    }

    if wants(ExportDataset::Contracts) {
        let rows = company_queries::list_contracts(
            &state.db,
            &ContractQuery {
                company_id: params.company_id,
                status: None,
                current_only: false,
            },
        )
        .await?
        .into_iter()
        .map(|c| {
            vec![
                c.company_name.into(),
                c.contract_number.into(),
                c.start_date.to_string().into(),
                c.end_date.to_string().into(),
                c.status.label().into(),
                c.days_left.into(),
                c.terms.into(),
            ]
        })
        .collect();
        write_sheet(
            &mut workbook,
            "ხელშეკრულებები",
            &[
                "კომპანია",
                "ნომერი",
                "დაწყება",
                "დასრულება",
                "სტატუსი",
                "დარჩენილი დღე",
                "პირობები",
            ],
            rows,
        )?;
    }

    if wants(ExportDataset::Tariffs) {
        let rows = company_queries::tariff_history(&state.db, params.company_id)
            .await?
            .into_iter()
            .map(|t| {
                vec![
                    name_of(t.company_id).into(),
                    t.waste_type.label().into(),
                    t.price_per_kg.into(),
                    t.fixed_fee.into(),
                    t.effective_from.to_string().into(),
                    t.note.into(),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "ტარიფები",
            &[
                "კომპანია",
                "ნარჩენის ტიპი",
                "ფასი 1 კგ (₾)",
                "ფიქსირებული საფასური (₾)",
                "მოქმედებს",
                "შენიშვნა",
            ],
            rows,
        )?;
    }

    if wants(ExportDataset::Pickups) {
        let (pickups, ..) = company_pickup_queries::search_pickups(
            &state.db,
            &PickupQuery {
                company_id: params.company_id,
                waste_type: None,
                status: None,
                from: params.from,
                to: params.to,
                limit: None,
                offset: None,
            },
            false,
        )
        .await?;
        let rows = pickups
            .into_iter()
            .map(|p| {
                vec![
                    p.id.into(),
                    p.pickup_date.to_string().into(),
                    p.company_name.into(),
                    p.address_snapshot.into(),
                    p.waste_type.label().into(),
                    p.quantity_kg.into(),
                    p.unit_price.map(Cell::from).unwrap_or_else(|| "".into()),
                    p.amount.into(),
                    p.status.label().into(),
                    p.invoice_number.into(),
                    p.created_by_name.into(),
                    p.notes.into(),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "გატანები",
            &[
                "ID",
                "თარიღი",
                "კომპანია",
                "მისამართი",
                "ნარჩენის ტიპი",
                "რაოდენობა (კგ)",
                "ფასი 1 კგ (₾)",
                "ღირებულება (₾)",
                "სტატუსი",
                "ინვოისი",
                "დააფიქსირა",
                "შენიშვნა",
            ],
            rows,
        )?;
    }

    if wants(ExportDataset::WasteTotals) {
        let rows = company_queries::per_company_totals(&state.db, &report_query)
            .await?
            .into_iter()
            .map(|t| {
                vec![
                    t.company_name.into(),
                    t.plastic_kg.into(),
                    t.paper_kg.into(),
                    t.aluminum_kg.into(),
                    t.total_kg.into(),
                    t.pickups_count.into(),
                    t.amount.into(),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "ნარჩენების რაოდენობა",
            &[
                "კომპანია",
                "პლასტმასი (კგ)",
                "ქაღალდი (კგ)",
                "ალუმინი (კგ)",
                "სულ (კგ)",
                "გატანები",
                "ღირებულება (₾)",
            ],
            rows,
        )?;
    }

    if wants(ExportDataset::Invoices) {
        let (invoices, ..) = company_pickup_queries::search_invoices(
            &state.db,
            &InvoiceQuery {
                company_id: params.company_id,
                status: None,
                from: params.from,
                to: params.to,
                limit: None,
                offset: None,
            },
            false,
        )
        .await?;
        let rows = invoices
            .into_iter()
            .map(|i| {
                vec![
                    i.invoice_number.into(),
                    i.company_name.into(),
                    i.issue_date.to_string().into(),
                    i.due_date.to_string().into(),
                    i.pickup_date.to_string().into(),
                    i.waste_type.label().into(),
                    i.quantity_kg.into(),
                    i.total.into(),
                    i.status.label().into(),
                    opt_date(i.paid_at.map(|d| d.format("%Y-%m-%d").to_string())),
                    opt_date(i.last_sent_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string())),
                ]
            })
            .collect();
        write_sheet(
            &mut workbook,
            "ინვოისები",
            &[
                "ნომერი",
                "კომპანია",
                "გამოწერის თარიღი",
                "გადახდის ვადა",
                "გატანის თარიღი",
                "ნარჩენის ტიპი",
                "რაოდენობა (კგ)",
                "თანხა (₾)",
                "სტატუსი",
                "გადახდის თარიღი",
                "ბოლოს გაგზავნილი",
            ],
            rows,
        )?;
    }

    let buffer = workbook
        .save_to_buffer()
        .map_err(|e| AppError::InternalError(format!("excel-ის შენახვა ვერ მოხერხდა: {}", e)))?;

    let headers = [
        (
            http::header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        (
            http::header::CONTENT_DISPOSITION,
            "attachment; filename=\"companies.xlsx\"",
        ),
    ];

    Ok((headers, buffer).into_response())
}
