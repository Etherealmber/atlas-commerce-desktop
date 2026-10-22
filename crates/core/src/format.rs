//! French-first formatting helpers (numbers, currency, dates).

/// Formats an amount in the shop's currency (MAD unless the company settings
/// say otherwise), with the French thousands separator.
#[must_use]
pub fn money(value: f64, currency: &str) -> String {
    let currency = if currency.trim().is_empty() {
        "MAD"
    } else {
        currency.trim()
    };
    format!("{} {currency}", number(value))
}

/// Formats a number with a narrow no-break space as the thousands separator
/// and at most two decimals, French style (comma decimal).
#[must_use]
pub fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        integer(value.round() as i64)
    } else {
        let rounded = (value * 100.0).round() / 100.0;
        let (whole, fraction) = if rounded < 0.0 {
            // Negative stock never really happens, but stay correct.
            let abs = rounded.abs();
            (-1, format!("{abs:.2}"))
        } else {
            (1, format!("{rounded:.2}"))
        };
        let formatted = fraction.replace('.', ",");
        let (int_part, frac_part) = formatted.split_once(',').unwrap_or((&formatted, ""));
        let grouped = group_integer(int_part);
        if whole < 0 {
            format!("-{grouped},{frac_part}")
        } else {
            format!("{grouped},{frac_part}")
        }
    }
}

#[must_use]
pub fn integer(value: i64) -> String {
    let negative = value < 0;
    let grouped = group_integer(&value.unsigned_abs().to_string());
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

fn group_integer(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index) % 3 == 0 {
            out.push(' ');
        }
        out.push(*byte as char);
    }
    out
}

/// Trims an ISO 8601 timestamp to `jj/mm/aaaa` for compact tables, keeping
/// the original when it does not look like a date.
#[must_use]
pub fn short_date(iso: &str) -> String {
    let Some(date) = iso.split('T').next() else {
        return iso.to_owned();
    };
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() == 3 && parts.iter().all(|part| part.chars().all(char::is_numeric)) {
        format!("{}/{}/{}", parts[2], parts[1], &parts[0][2..])
    } else {
        iso.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_numbers() {
        assert_eq!(number(1250.0), "1 250");
        assert_eq!(number(1250.5), "1 250,50");
        assert_eq!(integer(-42), "-42");
        assert_eq!(money(3_400_000.0, "MAD"), "3 400 000 MAD");
        assert_eq!(money(10.0, ""), "10 MAD");
    }

    #[test]
    fn compact_dates() {
        assert_eq!(short_date("2026-10-22T09:30:00Z"), "22/10/26");
        assert_eq!(short_date("hier"), "hier");
    }
}
