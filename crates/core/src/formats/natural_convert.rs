//! Natural conversion-query format.
//!
//! Recognizes free-text conversion phrases of the shape
//! `<value><unit> (to|in) <unit>` and answers them directly, e.g.
//!
//! - `5 km in miles`   → `3.11 mi`
//! - `100 USD to EUR`  → `92.34 EUR` (subject to cached rates)
//! - `5km to mi`       → `3.11 mi`
//!
//! The phrase is unambiguous, so a match yields high confidence (0.95).
//!
//! ## How it works (and why it's DRY)
//!
//! Rather than re-implement unit tables, this format delegates to the existing
//! per-family parsers:
//!
//! 1. Split the input on ` to ` / ` in ` (case-insensitive). The **left** side
//!    must have a value+unit shape, so plain prose containing " in " (e.g.
//!    "log in now") never matches — we require the left side to parse as a
//!    known quantity.
//! 2. Parse the left side with each unit family's `Format::parse` (and the
//!    currency parser). Whichever family recognizes it wins.
//! 3. For units, parse `"1 <target>"` with the **same** family to obtain the
//!    target unit's size in base units — reusing that family's full alias table
//!    for free. The answer is then `base_value / target_base`.
//! 4. For currency, route through the shared `RateCache`, degrading gracefully
//!    (no answer, no error) when rates are unavailable offline.
//!
//! An unknown target unit makes the phrase fall through (no interpretation,
//! no hard error), so the input can still be read as ordinary text.

use crate::format::{Format, FormatInfo};
use crate::formats::currency::CurrencyFormat;
use crate::formats::currency_rates::RateCache;
use crate::formats::temperature::TemperatureFormat;
use crate::formats::units::{
    format_value, AngleFormat, AreaFormat, EnergyFormat, LengthFormat, PressureFormat, SpeedFormat,
    VolumeFormat, WeightFormat,
};
use crate::types::{Conversion, ConversionPriority, ConversionStep, CoreValue, Interpretation};

#[derive(Default)]
pub struct NaturalConvertFormat;

/// Result of resolving a conversion query.
struct QueryAnswer {
    /// The target value, in the family's base unit / target currency.
    value: CoreValue,
    /// Fully rendered answer, e.g. `"3.11 mi"` or `"€87.49"`.
    /// `None` when unavailable (e.g. currency rates offline).
    result: Option<String>,
    /// One-line summary for the interpretation header, e.g. `"5 km = 3.11 mi"`.
    description: String,
}

/// Enumerate candidate `(left, target)` splits of `<left> to|in <target>`.
///
/// Connectors must be surrounded by spaces (so "into"/"tonne" don't match).
/// Every occurrence of every connector yields a candidate, because a connector
/// word can also be a unit: in `5 in to cm`, the ` in ` occurrence is the
/// inches suffix and the real connector is ` to `. The resolver tries the
/// candidates in order and keeps the first that actually resolves.
fn split_candidates(input: &str) -> Vec<(&str, &str)> {
    let lower = input.to_lowercase();
    // If lowercasing changed byte offsets (exotic Unicode), search the raw
    // input instead — conversion queries are effectively ASCII anyway.
    let haystack: &str = if lower.len() == input.len() {
        &lower
    } else {
        input
    };

    let mut candidates = Vec::new();
    for conn in [" to ", " in ", " into ", " as "] {
        for (pos, _) in haystack.match_indices(conn) {
            // Guard against char-boundary mismatch between `lower` and `input`.
            if !input.is_char_boundary(pos) || !input.is_char_boundary(pos + conn.len()) {
                continue;
            }
            let left = input[..pos].trim();
            let right = input[pos + conn.len()..].trim();
            if left.is_empty() || right.is_empty() {
                continue;
            }
            // The target must be a short unit/currency token (allow two words
            // for "nautical miles" / "fluid ounces"), not a whole sentence.
            if right.split_whitespace().count() > 2 {
                continue;
            }
            candidates.push((left, right));
        }
    }
    candidates
}

/// Try to parse `left` as a quantity with the given unit `format`, returning
/// its base-unit magnitude if it belongs to that family.
fn parse_base(format: &dyn Format, left: &str) -> Option<f64> {
    let interp = format.parse(left).into_iter().next()?;
    base_magnitude(&interp.value)
}

/// Extract the scalar base magnitude from a unit `CoreValue`.
fn base_magnitude(value: &CoreValue) -> Option<f64> {
    match value {
        CoreValue::Length(v)
        | CoreValue::Weight(v)
        | CoreValue::Volume(v)
        | CoreValue::Speed(v)
        | CoreValue::Pressure(v)
        | CoreValue::Energy(v)
        | CoreValue::Angle(v)
        | CoreValue::Area(v)
        | CoreValue::Temperature(v) => Some(*v),
        _ => None,
    }
}

/// Rebuild a unit `CoreValue` of the same variant as `template` from a base
/// magnitude. Used to hand the target value back to the graph so the family's
/// own conversions can enumerate the usual representations.
fn rebuild_like(template: &CoreValue, base: f64) -> Option<CoreValue> {
    Some(match template {
        CoreValue::Length(_) => CoreValue::Length(base),
        CoreValue::Weight(_) => CoreValue::Weight(base),
        CoreValue::Volume(_) => CoreValue::Volume(base),
        CoreValue::Speed(_) => CoreValue::Speed(base),
        CoreValue::Pressure(_) => CoreValue::Pressure(base),
        CoreValue::Energy(_) => CoreValue::Energy(base),
        CoreValue::Angle(_) => CoreValue::Angle(base),
        CoreValue::Area(_) => CoreValue::Area(base),
        CoreValue::Temperature(_) => CoreValue::Temperature(base),
        _ => return None,
    })
}

impl NaturalConvertFormat {
    /// The unit families we support, in a stable probe order.
    fn unit_families() -> Vec<Box<dyn Format>> {
        vec![
            Box::new(LengthFormat),
            Box::new(WeightFormat),
            Box::new(VolumeFormat),
            Box::new(SpeedFormat),
            Box::new(PressureFormat),
            Box::new(EnergyFormat),
            Box::new(AngleFormat),
            Box::new(AreaFormat),
            Box::new(TemperatureFormat),
        ]
    }

    /// Resolve a unit conversion query. Temperature is handled specially since
    /// it is affine (offset), not a simple ratio.
    fn resolve_unit(left: &str, target: &str) -> Option<QueryAnswer> {
        for family in Self::unit_families() {
            let interp = family.parse(left).into_iter().next();
            let Some(interp) = interp else {
                continue;
            };
            let Some(source_base) = base_magnitude(&interp.value) else {
                continue;
            };

            // Temperature is affine: convert via the family's own parser by
            // asking it to parse a value expressed in the target unit, then
            // solving. We reuse `1<unit>` and `2<unit>` to derive scale+offset.
            if matches!(interp.value, CoreValue::Temperature(_)) {
                return Self::resolve_temperature(&*family, source_base, left, target);
            }

            // Ratio families: parse "1 <target>" to learn the target unit size
            // in base units (reusing the family's full alias table).
            let one_target = format!("1 {target}");
            let one_target_nospace = format!("1{target}");
            let target_base = parse_base(&*family, &one_target)
                .or_else(|| parse_base(&*family, &one_target_nospace))?;
            if target_base == 0.0 {
                return None;
            }

            let converted = source_base / target_base;
            let target_display = format!("{} {}", format_value(converted), target.trim());
            let value = rebuild_like(&interp.value, source_base)?;
            // Echo the user's own left side ("5 km"), not the base-unit form.
            let description = format!("{} = {}", left.trim(), target_display);
            return Some(QueryAnswer {
                value,
                result: Some(target_display),
                description,
            });
        }
        None
    }

    /// Affine (temperature) conversion. We derive the target unit's scale and
    /// offset relative to Kelvin by parsing two known points ("0<unit>" and
    /// "100<unit>") with the family's own parser.
    fn resolve_temperature(
        family: &dyn Format,
        source_kelvin: f64,
        left: &str,
        target: &str,
    ) -> Option<QueryAnswer> {
        let p0 = parse_base(family, &format!("0{target}"))
            .or_else(|| parse_base(family, &format!("0 {target}")))?;
        let p100 = parse_base(family, &format!("100{target}"))
            .or_else(|| parse_base(family, &format!("100 {target}")))?;
        let scale = (p100 - p0) / 100.0; // Kelvin per target degree
        if scale == 0.0 {
            return None;
        }
        let converted = (source_kelvin - p0) / scale;
        let target_display = format!("{} {}", format_value(converted), target.trim());
        let description = format!("{} = {}", left.trim(), target_display);
        Some(QueryAnswer {
            value: CoreValue::Temperature(source_kelvin),
            result: Some(target_display),
            description,
        })
    }

    /// Resolve a currency conversion query. Degrades gracefully offline.
    fn resolve_currency(left: &str, target: &str) -> Option<QueryAnswer> {
        // Parse the left side as a currency amount.
        let interp = CurrencyFormat.parse(left).into_iter().next()?;
        let CoreValue::Currency { amount, code } = interp.value else {
            return None;
        };

        let target_code = target.trim().to_uppercase();

        // Validate the target code statically (works offline too);
        // unknown target → fall through, no hard error.
        if !crate::formats::currency::is_known_currency(&target_code) {
            return None;
        }

        let Some(cache) = RateCache::get() else {
            // No cached rates and offline: say so rather than erroring.
            let description = format!(
                "{} to {}: no exchange rates available (offline)",
                CurrencyFormat::format_amount(amount, &code),
                target_code
            );
            return Some(QueryAnswer {
                value: CoreValue::Currency {
                    amount,
                    code: code.clone(),
                },
                result: None,
                description,
            });
        };

        // Validate target currency; unknown target → fall through.
        if !cache.has_currency(&target_code) {
            return None;
        }

        let converted = cache.convert(amount, &code, &target_code)?;
        let target_display = CurrencyFormat::format_amount(converted, &target_code);
        let description = format!(
            "{} = {}",
            CurrencyFormat::format_amount(amount, &code),
            target_display
        );
        Some(QueryAnswer {
            value: CoreValue::Currency {
                amount: converted,
                code: target_code,
            },
            result: Some(target_display),
            description,
        })
    }

    /// Resolve a full query string. Returns `None` if it is not a conversion
    /// query or the target is unknown.
    fn resolve(input: &str) -> Option<QueryAnswer> {
        for (left, target) in split_candidates(input) {
            // Units first (unambiguous shapes), then currency.
            if let Some(answer) =
                Self::resolve_unit(left, target).or_else(|| Self::resolve_currency(left, target))
            {
                return Some(answer);
            }
        }
        None
    }
}

impl Format for NaturalConvertFormat {
    fn id(&self) -> &'static str {
        "convert-query"
    }

    fn name(&self) -> &'static str {
        "Conversion Query"
    }

    fn info(&self) -> FormatInfo {
        FormatInfo {
            id: self.id(),
            name: self.name(),
            category: "Units",
            description: "Natural conversion queries like '5 km in miles' or '100 USD to EUR'",
            examples: &["5 km in miles", "100 USD to EUR", "5km to mi", "72F to C"],
            aliases: self.aliases(),
            has_validation: false,
        }
    }

    fn parse(&self, input: &str) -> Vec<Interpretation> {
        let Some(answer) = Self::resolve(input) else {
            return vec![];
        };

        vec![Interpretation {
            value: answer.value,
            source_format: "convert-query".to_string(),
            confidence: 0.95,
            description: answer.description,
            rich_display: vec![],
        }]
    }

    fn can_format(&self, _value: &CoreValue) -> bool {
        false
    }

    fn format(&self, _value: &CoreValue) -> Option<String> {
        None
    }

    fn conversions(&self, _value: &CoreValue) -> Vec<Conversion> {
        // Don't emit from here — only when this format was the source parser.
        vec![]
    }

    fn source_conversions_for_input(&self, input: &str, value: &CoreValue) -> Vec<Conversion> {
        let Some(answer) = Self::resolve(input) else {
            return vec![];
        };
        if &answer.value != value {
            return vec![];
        }
        let Some(display) = answer.result else {
            return vec![];
        };

        vec![Conversion::new(value.clone(), "result", display.clone())
            .steps(vec![ConversionStep {
                format: "result".to_string(),
                value: value.clone(),
                display,
            }])
            .priority(ConversionPriority::Primary)
            .display_only(true)]
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["convert", "conversion"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(input: &str) -> Option<String> {
        NaturalConvertFormat::resolve(input).and_then(|a| a.result)
    }

    #[test]
    fn length_queries() {
        // 5 km = 3.10686 mi
        let a = answer("5 km in miles").expect("should resolve");
        assert!(a.contains("3.1"), "got {a}");
        let b = answer("5km to mi").expect("should resolve");
        assert!(b.contains("3.1"), "got {b}");
        // 1 mile = 1.609 km
        let c = answer("1 mile to km").expect("should resolve");
        assert!(c.contains("1.6"), "got {c}");
    }

    #[test]
    fn weight_and_speed_queries() {
        let w = answer("10 kg in pounds").expect("should resolve");
        assert!(w.contains("22"), "got {w}"); // ~22.05 lb
        let s = answer("100 kph to mph").expect("should resolve");
        assert!(s.contains("62"), "got {s}"); // ~62.1 mph
    }

    #[test]
    fn temperature_query() {
        // 100 C = 212 F
        let t = answer("100C to F").expect("should resolve");
        assert!(t.contains("212"), "got {t}");
        // 32 F = 0 C
        let t2 = answer("32F to C").expect("should resolve");
        assert!(t2.starts_with('0') || t2.contains(" 0"), "got {t2}");
    }

    #[test]
    fn unknown_target_falls_through() {
        assert!(answer("5 km in bananas").is_none());
        assert!(answer("5 km in").is_none());
    }

    #[test]
    fn connector_word_can_also_be_a_unit() {
        // "in" here is the inches unit; the real connector is "to".
        let a = answer("5 in to cm").expect("should resolve");
        assert!(a.contains("12.7"), "got {a}"); // 5 in = 12.7 cm
    }

    #[test]
    fn prose_does_not_match() {
        let format = NaturalConvertFormat;
        // No value+unit on the left → must not match.
        assert!(format.parse("log in now").is_empty());
        assert!(format.parse("please sign in here").is_empty());
        assert!(format.parse("checked in to the hotel").is_empty());
        // "to" as a plain word.
        assert!(format.parse("I went to the store").is_empty());
    }

    #[test]
    fn high_confidence_on_match() {
        let format = NaturalConvertFormat;
        let interps = format.parse("5 km in miles");
        assert_eq!(interps.len(), 1);
        assert!((interps[0].confidence - 0.95).abs() < 1e-6);
    }

    #[test]
    fn source_conversions_emit_primary_result() {
        let format = NaturalConvertFormat;
        let interps = format.parse("5 km in miles");
        assert_eq!(interps.len(), 1);

        let convs = format.source_conversions_for_input("5 km in miles", &interps[0].value);
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].target_format, "result");
        assert_eq!(convs[0].priority, ConversionPriority::Primary);
        assert!(convs[0].display.contains("3.1"), "got {}", convs[0].display);

        // A different value must not receive this input's result.
        let unrelated = CoreValue::Length(42.0);
        assert!(format
            .source_conversions_for_input("5 km in miles", &unrelated)
            .is_empty());
    }

    #[test]
    fn source_conversions_are_request_local() {
        let format = NaturalConvertFormat;
        let value = CoreValue::Length(5000.0);

        let miles = format.source_conversions_for_input("5 km to miles", &value);
        let feet = format.source_conversions_for_input("5 km to feet", &value);

        assert!(miles[0].display.contains("miles"));
        assert!(feet[0].display.contains("feet"));
    }

    #[test]
    fn convert_all_keeps_results_request_local() {
        let forb = crate::Formatorbit::new();

        let result = |input: &str| {
            forb.convert_all(input)
                .into_iter()
                .find(|r| r.interpretation.source_format == "convert-query")
                .and_then(|r| {
                    r.conversions
                        .into_iter()
                        .find(|c| c.target_format == "result")
                })
                .map(|c| c.display)
                .expect("conversion query should emit a primary result")
        };

        assert!(result("5 km to miles").contains("miles"));
        assert!(result("5 km to feet").contains("feet"));
        assert!(result("5 km to miles").contains("miles"));
    }
}
