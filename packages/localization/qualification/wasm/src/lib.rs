use cantos_localization::{Currency, Formatter, TimeZone};
use wasm_bindgen::prelude::*;

fn error(value: impl std::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("{value:?}"))
}

#[wasm_bindgen]
pub fn decimal(tag: &str, value: &str) -> Result<String, JsValue> {
    Formatter::try_new(tag).and_then(|formatter| formatter.number(value)).map_err(error)
}

#[wasm_bindgen]
pub fn cardinal(tag: &str, value: &str) -> Result<String, JsValue> {
    Formatter::try_new(tag).and_then(|formatter| formatter.cardinal(value)).map(|category| format!("{category:?}")).map_err(error)
}

#[wasm_bindgen]
pub fn money(tag: &str, units: &str, code: &str) -> Result<String, JsValue> {
    let units = units.parse::<u64>().map_err(error)?;
    let currency = match code {
        "USD" => Currency::Usd,
        "VND" => Currency::Vnd,
        _ => return Err(JsValue::from_str("UnsupportedCurrency")),
    };
    Formatter::try_new(tag).and_then(|formatter| formatter.currency(units, currency)).map_err(error)
}

#[wasm_bindgen]
pub fn instant(tag: &str, unix_ms: &str, offset_minutes: i16) -> Result<String, JsValue> {
    let unix_ms = unix_ms.parse::<i64>().map_err(error)?;
    Formatter::try_new(tag).and_then(|formatter| formatter.date_time(unix_ms, TimeZone::OffsetMinutes(offset_minutes))).map_err(error)
}

#[wasm_bindgen]
pub fn clock(tag: &str, seconds: u32) -> Result<String, JsValue> {
    Formatter::try_new(tag).map(|formatter| formatter.clock(u64::from(seconds))).map_err(error)
}
