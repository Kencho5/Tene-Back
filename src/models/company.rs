use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CompanyType {
    Legal,
    Individual,
}

impl CompanyType {
    pub fn label(&self) -> &'static str {
        match self {
            CompanyType::Legal => "იურიდიული პირი",
            CompanyType::Individual => "ფიზიკური პირი",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum WasteType {
    Plastic,
    Paper,
    Aluminum,
}

impl WasteType {
    pub const ALL: [WasteType; 3] = [WasteType::Plastic, WasteType::Paper, WasteType::Aluminum];

    pub fn label(&self) -> &'static str {
        match self {
            WasteType::Plastic => "პლასტმასი",
            WasteType::Paper => "ქაღალდი",
            WasteType::Aluminum => "ალუმინი",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Active,
    LessActive,
    Inactive,
}

impl ActivityStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ActivityStatus::Active => "აქტიური",
            ActivityStatus::LessActive => "ნაკლებად აქტიური",
            ActivityStatus::Inactive => "არააქტიური",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Upcoming,
    Active,
    Expiring,
    Expired,
}

impl ContractStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ContractStatus::Upcoming => "დასაწყები",
            ContractStatus::Active => "მოქმედი",
            ContractStatus::Expiring => "იწურება",
            ContractStatus::Expired => "ვადაგასული",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PickupStatus {
    Pending,
    Confirmed,
}

impl PickupStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PickupStatus::Pending => "დასადასტურებელი",
            PickupStatus::Confirmed => "დადასტურებული",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum InvoiceStatus {
    Unpaid,
    Sent,
    Paid,
    Overdue,
}

impl InvoiceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            InvoiceStatus::Unpaid => "გადაუხდელი",
            InvoiceStatus::Sent => "გაგზავნილი",
            InvoiceStatus::Paid => "გადახდილი",
            InvoiceStatus::Overdue => "ვადაგადაცილებული",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Company {
    pub id: i32,
    pub name: String,
    pub identification_code: String,
    pub company_type: CompanyType,
    pub is_active: bool,
    pub payment_terms_days: i32,
    pub payment_terms: Option<String>,
    pub requires_confirmation: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanySummary {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub company: Company,
    pub activity_status: ActivityStatus,
    pub last_pickup_date: Option<NaiveDate>,
    pub pickups_recent: i64,
    pub contract_start_date: Option<NaiveDate>,
    pub contract_end_date: Option<NaiveDate>,
    pub contract_days_left: Option<i32>,
    pub unpaid_invoices_count: i64,
    pub unpaid_invoices_total: Decimal,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyAddress {
    pub id: i32,
    pub company_id: i32,
    pub label: Option<String>,
    pub city: Option<String>,
    pub address: String,
    pub created_at: DateTime<Utc>,
}

impl CompanyAddress {
    pub fn display(&self) -> String {
        [self.label.as_deref(), self.city.as_deref(), Some(self.address.as_str())]
            .into_iter()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyContact {
    pub id: i32,
    pub company_id: i32,
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub receives_invoices: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyContract {
    pub id: i32,
    pub company_id: i32,
    pub company_name: String,
    pub contract_number: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub terms: Option<String>,
    #[serde(skip)]
    pub file_uuid: Option<Uuid>,
    #[serde(skip)]
    pub file_extension: Option<String>,
    pub file_name: Option<String>,
    pub status: ContractStatus,
    pub days_left: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CompanyContractResponse {
    #[serde(flatten)]
    pub contract: CompanyContract,
    pub file_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyTariff {
    pub id: i32,
    pub company_id: i32,
    pub waste_type: WasteType,
    pub price_per_kg: Decimal,
    pub fixed_fee: Decimal,
    pub effective_from: NaiveDate,
    pub note: Option<String>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CompanyDetail {
    #[serde(flatten)]
    pub summary: CompanySummary,
    pub addresses: Vec<CompanyAddress>,
    pub contacts: Vec<CompanyContact>,
    pub contracts: Vec<CompanyContractResponse>,
    pub current_tariffs: Vec<CompanyTariff>,
    pub tariff_history: Vec<CompanyTariff>,
}

#[derive(Debug, Serialize)]
pub struct CompanySearchResponse {
    pub companies: Vec<CompanySummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct CompanyQuery {
    pub query: Option<String>,
    pub activity_status: Option<ActivityStatus>,
    pub is_active: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyRequest {
    pub name: String,
    pub identification_code: String,
    pub company_type: CompanyType,
    pub is_active: Option<bool>,
    pub payment_terms_days: Option<i32>,
    pub payment_terms: Option<String>,
    pub requires_confirmation: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyAddressRequest {
    pub label: Option<String>,
    pub city: Option<String>,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct CompanyContactRequest {
    pub full_name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    #[serde(default)]
    pub receives_invoices: bool,
}

#[derive(Debug, Deserialize)]
pub struct CompanyContractRequest {
    pub contract_number: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub terms: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContractQuery {
    pub company_id: Option<i32>,
    pub status: Option<ContractStatus>,
    #[serde(default)]
    pub current_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContractFileUploadRequest {
    pub content_type: String,
    pub file_name: String,
}

#[derive(Debug, Serialize)]
pub struct ContractFileUploadResponse {
    pub upload_url: String,
}

#[derive(Debug, Deserialize)]
pub struct CompanyTariffRequest {
    pub waste_type: WasteType,
    pub price_per_kg: Decimal,
    pub fixed_fee: Option<Decimal>,
    pub effective_from: NaiveDate,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyPickup {
    pub id: i32,
    pub company_id: i32,
    pub company_name: String,
    pub address_id: Option<i32>,
    pub address_snapshot: Option<String>,
    pub pickup_date: NaiveDate,
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub tariff_id: Option<i32>,
    pub unit_price: Option<Decimal>,
    pub fixed_fee: Decimal,
    pub amount: Decimal,
    pub amount_overridden: bool,
    pub status: PickupStatus,
    pub notes: Option<String>,
    pub created_by_name: Option<String>,
    pub confirmed_by_name: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub invoice_id: Option<i32>,
    pub invoice_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyPickupRequest {
    pub company_id: i32,
    pub address_id: Option<i32>,
    pub pickup_date: NaiveDate,
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub amount: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PickupQuery {
    pub company_id: Option<i32>,
    pub waste_type: Option<WasteType>,
    pub status: Option<PickupStatus>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PickupSearchResponse {
    pub pickups: Vec<CompanyPickup>,
    pub total: i64,
    pub total_quantity_kg: Decimal,
    pub total_amount: Decimal,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct PickupConfirmResponse {
    pub pickup: CompanyPickup,
    pub invoice: Option<CompanyInvoice>,
    pub delivery: Option<InvoiceDelivery>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyInvoice {
    pub id: i32,
    pub invoice_number: String,
    pub company_id: i32,
    pub company_name: String,
    pub pickup_id: i32,
    pub pickup_date: NaiveDate,
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total: Decimal,
    pub status: InvoiceStatus,
    pub paid_at: Option<DateTime<Utc>>,
    pub last_sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InvoiceDelivery {
    pub id: i32,
    pub invoice_id: i32,
    pub recipients: Vec<String>,
    pub success: bool,
    pub error: Option<String>,
    pub sent_by_name: Option<String>,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CompanyInvoiceDetail {
    #[serde(flatten)]
    pub invoice: CompanyInvoice,
    pub html: String,
    pub deliveries: Vec<InvoiceDelivery>,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceQuery {
    pub company_id: Option<i32>,
    pub status: Option<InvoiceStatus>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceSearchResponse {
    pub invoices: Vec<CompanyInvoice>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct InvoicePaymentRequest {
    pub paid: bool,
}

#[derive(Debug, Deserialize)]
pub struct CompanyReportQuery {
    pub company_id: Option<i32>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyExportQuery {
    pub company_id: Option<i32>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub dataset: Option<ExportDataset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDataset {
    Companies,
    Addresses,
    Contacts,
    Contracts,
    Tariffs,
    Pickups,
    WasteTotals,
    Invoices,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WasteTotal {
    pub waste_type: WasteType,
    pub quantity_kg: Decimal,
    pub amount: Decimal,
    pub pickups_count: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CompanyWasteTotal {
    pub company_id: i32,
    pub company_name: String,
    pub plastic_kg: Decimal,
    pub paper_kg: Decimal,
    pub aluminum_kg: Decimal,
    pub total_kg: Decimal,
    pub amount: Decimal,
    pub pickups_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct CompanyStatusCounts {
    pub companies_total: i64,
    pub active: i64,
    pub less_active: i64,
    pub inactive: i64,
    pub contracts_expired: i64,
    pub contracts_expiring: i64,
}

#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct InvoiceTotals {
    pub invoices_count: i64,
    pub total_amount: Decimal,
    pub paid_count: i64,
    pub paid_amount: Decimal,
    pub unpaid_count: i64,
    pub unpaid_amount: Decimal,
    pub overdue_count: i64,
    pub overdue_amount: Decimal,
}

#[derive(Debug, Serialize)]
pub struct CompanyReport {
    #[serde(flatten)]
    pub status_counts: CompanyStatusCounts,
    pub pickups_count: i64,
    pub pickups_pending: i64,
    pub waste_totals: Vec<WasteTotal>,
    pub per_company: Vec<CompanyWasteTotal>,
    pub invoices: InvoiceTotals,
}
