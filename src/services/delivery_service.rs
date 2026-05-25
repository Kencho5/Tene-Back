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

const TBILISI_SAME_DAY: Decimal = dec!(15);
const TBILISI_STANDARD: Decimal = dec!(6);
const HIGH_MOUNTAIN: Decimal = dec!(13.50);
const STANDARD: Decimal = dec!(8.50);

pub fn calculate_delivery(
    delivery_type: &str,
    delivery_time: &str,
    city: Option<&str>,
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
            TBILISI_SAME_DAY
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
