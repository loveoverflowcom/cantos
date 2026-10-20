//! Integer planning estimates: exact rational cost per dialogue, rounded up per line, summed
//! without overflow. A missing or invalid rate is unavailable, never zero.
use cantos_contracts::{
    ProductionBudget, ProductionCurrency, ProductionEstimate, ResolvedSynthesisInput,
};

pub const MAX_MONEY_MINOR: u64 = i64::MAX as u64;

/// Exact rational per-dialogue planning estimate, rounded upward for this one dialogue.
pub fn estimate_cost(
    currency: ProductionCurrency,
    units: u64,
    budget: &ProductionBudget,
) -> ProductionEstimate {
    let Some(rate) = &budget.rate else {
        return ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into(),
        };
    };
    if currency != budget.currency
        || units > MAX_MONEY_MINOR
        || rate.units_per_charge == 0
        || rate.units_per_charge > MAX_MONEY_MINOR
        || rate.amount_minor > MAX_MONEY_MINOR
    {
        return ProductionEstimate::Unavailable {
            reason: "invalid_or_out_of_range_rate".into(),
        };
    }
    let numerator = u128::from(units) * u128::from(rate.amount_minor);
    let denominator = u128::from(rate.units_per_charge);
    let rounded = numerator.div_ceil(denominator);
    if rounded > u128::from(MAX_MONEY_MINOR) {
        return ProductionEstimate::Unavailable {
            reason: "estimate_overflow".into(),
        };
    }
    ProductionEstimate::Known {
        currency,
        amount_minor: rounded as u64,
        units,
        rate_reference: rate.reference.clone(),
        rate_version: rate.version.clone(),
    }
}

/// A dialogue is the independent future send unit. Round each line before summing.
pub fn estimate_dialogues(
    lines: &[ResolvedSynthesisInput],
    budget: &ProductionBudget,
) -> ProductionEstimate {
    let mut total_units = 0_u64;
    let mut total_minor = 0_u64;
    for line in lines {
        let units = line.text.chars().count() as u64;
        match estimate_cost(budget.currency, units, budget) {
            ProductionEstimate::Unavailable { reason } => {
                return ProductionEstimate::Unavailable { reason }
            }
            ProductionEstimate::Known { amount_minor, .. } => {
                let Some(sum_units) = total_units
                    .checked_add(units)
                    .filter(|value| *value <= MAX_MONEY_MINOR)
                else {
                    return ProductionEstimate::Unavailable {
                        reason: "usage_overflow".into(),
                    };
                };
                let Some(sum_minor) = total_minor
                    .checked_add(amount_minor)
                    .filter(|value| *value <= MAX_MONEY_MINOR)
                else {
                    return ProductionEstimate::Unavailable {
                        reason: "estimate_overflow".into(),
                    };
                };
                total_units = sum_units;
                total_minor = sum_minor;
            }
        }
    }
    let Some(rate) = &budget.rate else {
        return ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into(),
        };
    };
    ProductionEstimate::Known {
        currency: budget.currency,
        amount_minor: total_minor,
        units: total_units,
        rate_reference: rate.reference.clone(),
        rate_version: rate.version.clone(),
    }
}
