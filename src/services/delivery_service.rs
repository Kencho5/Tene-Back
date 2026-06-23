use rust_decimal::{Decimal, dec};

use crate::error::{AppError, Result};

const TBILISI: &str = "tbilisi";
const HIGH_MOUNTAIN_CITIES: &[&str] = &[
    "svaneti",
    "racha",
    "khevsureti",
    "tusheti",
    "zemo-acshara",
];

const TBILISI_EXPRESS_NEAR: Decimal = dec!(15);
const TBILISI_EXPRESS_FAR: Decimal = dec!(25);
const TBILISI_STANDARD: Decimal = dec!(6);
const HIGH_MOUNTAIN: Decimal = dec!(8);
const STANDARD: Decimal = dec!(6);

/// Tbilisi express (same-day) price for a given district group (region).
/// Defaults to the far rate when the region is missing or unknown.
fn tbilisi_express_price(region: Option<&str>) -> Decimal {
    match region.unwrap_or("").trim().to_lowercase().as_str() {
        "vake-saburtalo" | "didube-chughureti" | "dzveli-tbilisi" => TBILISI_EXPRESS_NEAR,
        "isani-samgori" | "gldani-nadzaladevi" => TBILISI_EXPRESS_FAR,
        _ => TBILISI_EXPRESS_FAR,
    }
}

pub fn calculate_delivery(
    delivery_type: &str,
    delivery_time: &str,
    city: Option<&str>,
    region: Option<&str>,
) -> Result<Decimal> {
    if delivery_type == "pickup" {
        return Ok(Decimal::ZERO);
    }

    let city = city.unwrap_or("").trim().to_lowercase();
    let city = city.as_str();
    let is_tbilisi = city == TBILISI;
    let is_high_mountain = HIGH_MOUNTAIN_CITIES.contains(&city);

    if delivery_time == "same_day" && !is_tbilisi {
        return Err(AppError::BadRequest(
            "იმავე დღეს მიწოდება ხელმისაწვდომია მხოლოდ თბილისში".to_string(),
        ));
    }

    let cost = if is_tbilisi {
        if delivery_time == "same_day" {
            tbilisi_express_price(region)
        } else {
            TBILISI_STANDARD
        }
    } else if is_high_mountain {
        HIGH_MOUNTAIN
    } else {
        STANDARD
    };

    Ok(cost)
}
