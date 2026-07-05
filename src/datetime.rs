//! Date/time parsing and ANB formatting, mirroring `anxwritter/utils.py`.
//!
//! Accepts the same input forms the Python library does and emits the canonical
//! ANB datetime `YYYY-MM-DDTHH:MM:SS.000` (no fractional milliseconds). Used for
//! `<ChartItem>` `DateTime`/`DateSet`/`TimeSet`.

/// Parse a date in `YYYY-MM-DD`, `DD/MM/YYYY`, or `YYYYMMDD` form, with full
/// calendar validation (rejects e.g. month 13, day 32, bad leap days) — matching
/// Python's `_validate_date` (`_DATE_FORMATS` + `date()` construction). Ambiguous
/// US `MM/DD/YYYY` is intentionally not a supported form.
pub fn parse_date(s: &str) -> Option<(i64, u32, u32)> {
    use chrono::NaiveDate;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // (year, month, day) candidate by structural form; calendar-checked below.
    let ymd: (i64, u32, u32) = if s.contains('-') {
        // %Y-%m-%d
        let p: Vec<&str> = s.split('-').collect();
        if p.len() != 3 {
            return None;
        }
        (p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?)
    } else if s.contains('/') {
        // %d/%m/%Y
        let p: Vec<&str> = s.split('/').collect();
        if p.len() != 3 {
            return None;
        }
        (p[2].parse().ok()?, p[1].parse().ok()?, p[0].parse().ok()?)
    } else if s.len() == 8 && s.bytes().all(|c| c.is_ascii_digit()) {
        // %Y%m%d
        (
            s[0..4].parse().ok()?,
            s[4..6].parse().ok()?,
            s[6..8].parse().ok()?,
        )
    } else {
        return None;
    };
    // Real calendar validation (leap years, days-per-month, 1..=12 months).
    NaiveDate::from_ymd_opt(i32::try_from(ymd.0).ok()?, ymd.1, ymd.2)?;
    Some(ymd)
}

/// Parse a time in `HH:MM:SS[.ffffff]`, `HH:MM`, or `h:MM AM/PM` form, with range
/// validation (0–23 / 0–59) — matching Python's `_validate_time`.
pub fn parse_time(s: &str) -> Option<(u32, u32, u32)> {
    use chrono::NaiveTime;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (body, ampm) = match s.to_uppercase().rsplit_once(' ') {
        Some((b, suf)) if suf == "AM" || suf == "PM" => {
            (b.trim().to_string(), Some(suf.to_string()))
        }
        _ => (s.to_string(), None),
    };
    let mut parts = body.split(':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let sec: u32 = match parts.next() {
        Some(s) => s.split('.').next()?.parse().ok()?,
        None => 0,
    };
    // 12-hour forms: %I is 1..=12.
    if ampm.is_some() && !(1..=12).contains(&h) {
        return None;
    }
    let h = match ampm.as_deref() {
        Some("PM") if h < 12 => h + 12,
        Some("AM") if h == 12 => 0,
        _ => h,
    };
    // Range validation (0–23 / 0–59 / 0–59).
    NaiveTime::from_hms_opt(h, m, sec)?;
    Some((h, m, sec))
}

/// Build the ANB `DateTime` string plus `(date_set, time_set)` flags from
/// optional date/time inputs. Returns `None` when no date is present.
pub fn build_datetime(date: Option<&str>, time: Option<&str>) -> Option<(String, bool, bool)> {
    let d = date.and_then(parse_date)?;
    let t = time.and_then(parse_time);
    let (y, mo, da) = d;
    let (h, mi, se) = t.unwrap_or((0, 0, 0));
    let s = format!("{y:04}-{mo:02}-{da:02}T{h:02}:{mi:02}:{se:02}.000");
    Some((s, true, t.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_date_and_time() {
        assert_eq!(
            build_datetime(Some("2026-02-14"), Some("09:30:00")),
            Some(("2026-02-14T09:30:00.000".to_string(), true, true))
        );
    }

    #[test]
    fn date_only() {
        assert_eq!(
            build_datetime(Some("2026-02-15"), None),
            Some(("2026-02-15T00:00:00.000".to_string(), true, false))
        );
    }

    #[test]
    fn alt_formats() {
        assert_eq!(parse_date("14/02/2026"), Some((2026, 2, 14)));
        assert_eq!(parse_date("20260214"), Some((2026, 2, 14)));
        assert_eq!(parse_time("2:30 PM"), Some((14, 30, 0)));
    }
}
