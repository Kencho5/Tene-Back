use rust_decimal::{Decimal, dec};

use crate::error::{AppError, Result};

pub const FREE_SHIPPING_THRESHOLD: Decimal = dec!(100);

const TBILISI: &str = "თბილისი";
const HIGH_MOUNTAIN_CITIES: &[&str] = &[
    "სვანეთი",
    "რაჭა",
    "ხევსურეთი",
    "თუშეთი",
    "ზემო აჭარა",
];

const TBILISI_SAME_DAY: Decimal = dec!(12);
const TBILISI_STANDARD: Decimal = dec!(5.50);
const HIGH_MOUNTAIN: Decimal = dec!(13.50);
const STANDARD: Decimal = dec!(8.50);

pub fn calculate_delivery(
    delivery_type: &str,
    delivery_time: &str,
    city: Option<&str>,
    subtotal: Decimal,
) -> Result<Decimal> {
    if delivery_type == "pickup" {
        return Ok(Decimal::ZERO);
    }

    let city = city.unwrap_or("");
    let is_tbilisi = city == TBILISI;
    let is_high_mountain = HIGH_MOUNTAIN_CITIES.contains(&city);

    if delivery_time == "same_day" && !is_tbilisi {
        return Err(AppError::BadRequest(
            "იმავე დღეს მიწოდება ხელმისაწვდომია მხოლოდ თბილისში".to_string(),
        ));
    }

    if subtotal >= FREE_SHIPPING_THRESHOLD {
        return Ok(Decimal::ZERO);
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
