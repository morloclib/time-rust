use crate::mlc::types::time::{DateTime, TimeParse};

const TIME_RS_US: i128 = 1_000_000;
const TIME_RS_MINUTE: i128 = 60 * TIME_RS_US;
const TIME_RS_HOUR: i128 = 60 * TIME_RS_MINUTE;
const TIME_RS_DAY: i128 = 24 * TIME_RS_HOUR;

const TIME_RS_WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const TIME_RS_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn time_rs_ok<A>(x: A) -> Try<String, A> {
    Try::Ok((x,).into())
}

fn time_rs_err<A>(e: String) -> Try<String, A> {
    Try::Err((e,).into())
}

fn time_rs_in_i64(x: i128) -> bool {
    x >= i64::MIN as i128 && x <= i64::MAX as i128
}

// Python's repr of a str, for the texts quoted in error messages.
fn time_rs_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if c.is_control() => {
                let n = c as u32;
                if n < 0x100 {
                    out.push_str(&format!("\\x{:02x}", n));
                } else {
                    out.push_str(&format!("\\u{:04x}", n));
                }
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

// ---------------------------------------------------------------------------
// Civil dates, proleptic Gregorian, days counted from 1970-01-01

fn time_rs_is_leap(y: i128) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn time_rs_month_length(y: i128, m: i128) -> i128 {
    if m == 2 {
        if time_rs_is_leap(y) { 29 } else { 28 }
    } else if m == 4 || m == 6 || m == 9 || m == 11 {
        30
    } else {
        31
    }
}

fn time_rs_days_from_civil(y: i128, m: i128, d: i128) -> i128 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + 9).rem_euclid(12) + 2).div_euclid(5) + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn time_rs_civil_from_days(z: i128) -> (i128, i128, i128) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

fn time_rs_weekday_index(days: i128) -> i128 {
    (days + 3).rem_euclid(7)
}

// Python's "{:0Nd}": zero padding after the sign, to a total width of N.
fn time_rs_pad(digits: &str, neg: bool, width: usize) -> String {
    let w = if neg { width.saturating_sub(1) } else { width };
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    for _ in digits.len()..w {
        out.push('0');
    }
    out.push_str(digits);
    out
}

fn time_rs_pad_int(x: i128, width: usize) -> String {
    time_rs_pad(&x.unsigned_abs().to_string(), x < 0, width)
}

fn time_rs_year_text_int(y: i128) -> String {
    if y < 0 {
        format!("-{}", time_rs_pad_int(-y, 4))
    } else {
        time_rs_pad_int(y, 4)
    }
}

fn time_rs_date_error(y: i128, y04: &str, m: i128, d: i128) -> Option<String> {
    if !(1..=12).contains(&m) {
        return Some(format!("month {} is not 1 to 12", m));
    }
    if !(1 <= d && d <= time_rs_month_length(y, m)) {
        return Some(format!("day {} does not exist in {}-{}", d, y04, time_rs_pad_int(m, 2)));
    }
    None
}

fn time_rs_time_error(hour: i128, minute: i128, second: i128, micro: i128) -> Option<String> {
    for (field, v, top) in [
        ("hour", hour, 23),
        ("minute", minute, 59),
        ("second", second, 59),
        ("micro", micro, 999999),
    ] {
        if !(0 <= v && v <= top) {
            return Some(format!("{} {} is not 0 to {}", field, v, top));
        }
    }
    None
}

fn time_rs_split(offset: i128, t: i128) -> (i128, i128) {
    let x = t + offset * TIME_RS_MINUTE;
    (x.div_euclid(TIME_RS_DAY), x.rem_euclid(TIME_RS_DAY))
}

fn time_rs_instant(days: i128, us_of_day: i128, offset: i128) -> i128 {
    days * TIME_RS_DAY + us_of_day - offset * TIME_RS_MINUTE
}

pub fn time_rs_now() -> i64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    match d {
        Ok(d) => (d.as_nanos() / 1000) as i64,
        Err(e) => {
            let back = e.duration().as_nanos() as i128;
            (-back).div_euclid(1000) as i64
        }
    }
}

#[repr(C)]
struct TimeRsTm {
    tm_sec: std::os::raw::c_int,
    tm_min: std::os::raw::c_int,
    tm_hour: std::os::raw::c_int,
    tm_mday: std::os::raw::c_int,
    tm_mon: std::os::raw::c_int,
    tm_year: std::os::raw::c_int,
    tm_wday: std::os::raw::c_int,
    tm_yday: std::os::raw::c_int,
    tm_isdst: std::os::raw::c_int,
    tm_gmtoff: std::os::raw::c_long,
    tm_zone: *const std::os::raw::c_char,
}

extern "C" {
    fn tzset();
    fn localtime_r(t: *const i64, out: *mut TimeRsTm) -> *mut TimeRsTm;
}

#[cfg(not(all(unix, target_pointer_width = "64")))]
compile_error!("time-rust: localOffset needs a 64-bit Unix time_t and struct tm with tm_gmtoff");

pub fn time_rs_local_offset(t: i64) -> i32 {
    let secs: i64 = t.div_euclid(TIME_RS_US as i64);
    let mut tm = TimeRsTm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let r = unsafe {
        tzset();
        localtime_r(&secs, &mut tm)
    };
    if r.is_null() {
        rustmorloc::morloc_throw(format!("localOffset: the system cannot convert {} seconds to local time", secs));
    }
    (tm.tm_gmtoff as i64).div_euclid(60) as i32
}

pub fn time_rs_to_seconds(t: i64) -> i64 {
    t.div_euclid(TIME_RS_US as i64)
}

struct TimeRsCivil {
    year: i128,
    month: i128,
    day: i128,
    hour: i128,
    minute: i128,
    second: i128,
    micro: i128,
    doy: i128,
    weekday: i128,
}

fn time_rs_civil(offset: i128, t: i128) -> TimeRsCivil {
    let (days, rem) = time_rs_split(offset, t);
    let (y, m, d) = time_rs_civil_from_days(days);
    let secs = rem / TIME_RS_US;
    TimeRsCivil {
        year: y,
        month: m,
        day: d,
        hour: secs / 3600,
        minute: secs / 60 % 60,
        second: secs % 60,
        micro: rem % TIME_RS_US,
        doy: days - time_rs_days_from_civil(y, 1, 1) + 1,
        weekday: time_rs_weekday_index(days),
    }
}

pub fn time_rs_to_datetime(offset: i32, t: i64) -> DateTime {
    let c = time_rs_civil(offset as i128, t as i128);
    DateTime {
        year: c.year as i32,
        month: c.month as u8,
        day: c.day as u8,
        hour: c.hour as u8,
        minute: c.minute as u8,
        second: c.second as u8,
        micro: c.micro as u32,
    }
}

pub fn time_rs_from_datetime(offset: i32, dt: &DateTime) -> Try<String, i64> {
    let y = dt.year as i128;
    let y04 = time_rs_pad_int(y, 4);
    let (m, d) = (dt.month as i128, dt.day as i128);
    let e = time_rs_date_error(y, &y04, m, d)
        .or_else(|| time_rs_time_error(dt.hour as i128, dt.minute as i128, dt.second as i128, dt.micro as i128));
    if let Some(e) = e {
        return time_rs_err(e);
    }
    let days = time_rs_days_from_civil(y, m, d);
    let us_of_day = ((dt.hour as i128 * 60 + dt.minute as i128) * 60 + dt.second as i128) * TIME_RS_US
        + dt.micro as i128;
    let t = time_rs_instant(days, us_of_day, offset as i128);
    if !time_rs_in_i64(t) {
        return time_rs_err(format!(
            "{}-{}-{} is outside the range of an instant",
            y04,
            time_rs_pad_int(m, 2),
            time_rs_pad_int(d, 2)
        ));
    }
    time_rs_ok(t as i64)
}

pub fn time_rs_weekday(dt: &DateTime) -> Option<u8> {
    let y = dt.year as i128;
    let (m, d) = (dt.month as i128, dt.day as i128);
    if time_rs_date_error(y, "", m, d).is_some() {
        return None;
    }
    Some((time_rs_weekday_index(time_rs_days_from_civil(y, m, d)) + 1) as u8)
}

// ---------------------------------------------------------------------------
// Formats

#[derive(Clone, Copy, PartialEq)]
enum TimeRsTok {
    Lit(char),
    Space,
    Dir(char),
    ColonZ,
}

fn time_rs_tokens(fmt: &str) -> Result<Vec<TimeRsTok>, String> {
    let cs: Vec<char> = fmt.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c != '%' {
            out.push(if c == ' ' { TimeRsTok::Space } else { TimeRsTok::Lit(c) });
            i += 1;
            continue;
        }
        if i + 2 < cs.len() && cs[i + 1] == ':' && cs[i + 2] == 'z' {
            out.push(TimeRsTok::ColonZ);
            i += 3;
            continue;
        }
        if i + 1 >= cs.len() {
            return Err(format!("format ends with a lone % at character {}", i + 1));
        }
        let name = cs[i + 1];
        if !"YmdHMSjfsabz%".contains(name) {
            return Err(format!("unknown directive %{} at character {} of the format", name, i + 1));
        }
        out.push(TimeRsTok::Dir(name));
        i += 2;
    }
    Ok(out)
}

fn time_rs_fraction_text(f: i128, width: usize) -> String {
    if f == 0 {
        String::new()
    } else {
        format!(".{}", time_rs_pad_int(f, width).trim_end_matches('0'))
    }
}

fn time_rs_offset_text(offset: i128, colon: bool) -> String {
    let sign = if offset < 0 { "-" } else { "+" };
    let a = offset.abs();
    format!(
        "{}{}{}{}",
        sign,
        time_rs_pad_int(a / 60, 2),
        if colon { ":" } else { "" },
        time_rs_pad_int(a % 60, 2)
    )
}

pub fn time_rs_format_time(fmt: &String, offset: i32, t: i64) -> Try<String, String> {
    let tokens = match time_rs_tokens(fmt) {
        Ok(ts) => ts,
        Err(e) => return time_rs_err(e),
    };
    let offset = offset as i128;
    let c = time_rs_civil(offset, t as i128);
    let mut out = String::new();
    for tok in tokens {
        match tok {
            TimeRsTok::Lit(ch) => out.push(ch),
            TimeRsTok::Space => out.push(' '),
            TimeRsTok::ColonZ => out.push_str(&time_rs_offset_text(offset, true)),
            TimeRsTok::Dir(v) => {
                let s = match v {
                    'Y' => time_rs_year_text_int(c.year),
                    'm' => time_rs_pad_int(c.month, 2),
                    'd' => time_rs_pad_int(c.day, 2),
                    'H' => time_rs_pad_int(c.hour, 2),
                    'M' => time_rs_pad_int(c.minute, 2),
                    'S' => time_rs_pad_int(c.second, 2),
                    'j' => time_rs_pad_int(c.doy, 3),
                    'f' => time_rs_fraction_text(c.micro, 6),
                    's' => (t as i128).div_euclid(TIME_RS_US).to_string(),
                    'a' => TIME_RS_WEEKDAYS[c.weekday as usize].to_string(),
                    'b' => TIME_RS_MONTHS[(c.month - 1) as usize].to_string(),
                    'z' => time_rs_offset_text(offset, false),
                    _ => "%".to_string(),
                };
                out.push_str(&s);
            }
        }
    }
    time_rs_ok(out)
}

// A parsed integer. `text` is its exact decimal form; `v` is the value when
// it is at most 10^20 in size, and otherwise a stand-in of the same sign,
// beyond every instant, congruent to it modulo 400 (which fixes leap years
// and weekdays).
#[derive(Clone)]
struct TimeRsNum {
    v: i128,
    text: String,
}

const TIME_RS_BIG: i128 = 100_000_000_000_000_000_000;

impl TimeRsNum {
    fn small(v: i128) -> TimeRsNum {
        TimeRsNum { v, text: v.to_string() }
    }

    fn parse(neg: bool, digits: &str) -> TimeRsNum {
        let trimmed = digits.trim_start_matches('0');
        if trimmed.is_empty() {
            return TimeRsNum::small(0);
        }
        if trimmed.len() <= 21 {
            let a: i128 = trimmed.parse().unwrap();
            if a <= TIME_RS_BIG {
                return TimeRsNum::small(if neg { -a } else { a });
            }
        }
        let r: i128 = trimmed.bytes().fold(0i128, |acc, b| (acc * 10 + (b - b'0') as i128) % 400);
        let v = if neg { -TIME_RS_BIG + (400 - r) % 400 } else { TIME_RS_BIG + r };
        let text = if neg { format!("-{}", trimmed) } else { trimmed.to_string() };
        TimeRsNum { v, text }
    }

    fn is_neg(&self) -> bool {
        self.text.starts_with('-')
    }

    fn abs_digits(&self) -> &str {
        self.text.trim_start_matches('-')
    }

    fn year_text(&self) -> String {
        if self.is_neg() {
            format!("-{}", time_rs_pad(self.abs_digits(), false, 4))
        } else {
            time_rs_pad(self.abs_digits(), false, 4)
        }
    }

    fn pad04(&self) -> String {
        time_rs_pad(self.abs_digits(), self.is_neg(), 4)
    }
}

struct TimeRsReader {
    text: Vec<char>,
    orig: String,
    pos: usize,
}

impl TimeRsReader {
    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{} at character {} of {}", what, self.pos + 1, time_rs_repr(&self.orig)))
    }

    fn peek(&self) -> Option<char> {
        self.text.get(self.pos).copied()
    }

    fn peek_is(&self, c: char) -> bool {
        self.peek() == Some(c)
    }

    fn peek_digit(&self) -> bool {
        matches!(self.peek(), Some(c) if c.is_ascii_digit())
    }

    fn digits(&mut self, most: Option<usize>, directive: &str) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.text.len()
            && most.map_or(true, |m| self.pos - start < m)
            && self.text[self.pos].is_ascii_digit()
        {
            self.pos += 1;
        }
        if self.pos == start {
            return self.fail(&format!("expected digits for %{}", directive));
        }
        Ok(self.text[start..self.pos].iter().collect())
    }

    fn sign(&mut self, signs: &str) -> Option<char> {
        match self.peek() {
            Some(c) if signs.contains(c) => {
                self.pos += 1;
                Some(c)
            }
            _ => None,
        }
    }

    fn name(&mut self, names: &[&str], directive: &str) -> Result<usize, String> {
        let end = (self.pos + 3).min(self.text.len());
        let word: String = self.text[self.pos..end].iter().collect::<String>().to_lowercase();
        for (i, n) in names.iter().enumerate() {
            if word == n.to_lowercase() {
                self.pos += 3;
                return Ok(i);
            }
        }
        self.fail(&format!("expected a name for %{}", directive))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TimeRsKey {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Doy,
    Epoch,
    Micro,
    Weekday,
    Offset,
}

struct TimeRsFields {
    entries: Vec<(TimeRsKey, TimeRsNum, String)>,
}

impl TimeRsFields {
    fn get(&self, k: TimeRsKey) -> Option<&(TimeRsKey, TimeRsNum, String)> {
        self.entries.iter().find(|e| e.0 == k)
    }

    fn has(&self, k: TimeRsKey) -> bool {
        self.get(k).is_some()
    }

    fn val(&self, k: TimeRsKey, default: i128) -> TimeRsNum {
        self.get(k).map(|e| e.1.clone()).unwrap_or_else(|| TimeRsNum::small(default))
    }

    fn set(&mut self, k: TimeRsKey, value: TimeRsNum, directive: &str) -> Result<(), String> {
        if let Some(i) = self.entries.iter().position(|e| e.0 == k) {
            let (_, old, od) = &self.entries[i];
            if old.text != value.text {
                return Err(format!("%{} gives {} but %{} gave {}", directive, value.text, od, old.text));
            }
            self.entries[i] = (k, value, directive.to_string());
        } else {
            self.entries.push((k, value, directive.to_string()));
        }
        Ok(())
    }
}

fn time_rs_read_offset(r: &mut TimeRsReader, directive: &str) -> Result<i128, String> {
    if r.peek_is('Z') {
        r.pos += 1;
        return Ok(0);
    }
    let s = match r.sign("+-") {
        Some(s) => s,
        None => return r.fail(&format!("expected Z, + or - for %{}", directive)),
    };
    let hh = r.digits(Some(2), directive)?;
    if directive == ":z" {
        if !r.peek_is(':') {
            return r.fail("expected : in %:z");
        }
        r.pos += 1;
    }
    let mm = r.digits(Some(2), directive)?;
    let (h, m): (i128, i128) = (hh.parse().unwrap(), mm.parse().unwrap());
    if hh.chars().count() != 2 || mm.chars().count() != 2 || m > 59 {
        return r.fail(&format!("bad offset for %{}", directive));
    }
    let total = h * 60 + m;
    Ok(if s == '-' { -total } else { total })
}

fn time_rs_is_numeric(t: Option<&TimeRsTok>) -> bool {
    matches!(t, Some(TimeRsTok::Dir(c)) if "YmdHMSjs".contains(*c))
}

fn time_rs_scan(tokens: &[TimeRsTok], text: &str) -> Result<TimeRsFields, String> {
    let mut r = TimeRsReader { text: text.chars().collect(), orig: text.to_string(), pos: 0 };
    let mut f = TimeRsFields { entries: Vec::new() };
    for (n, tok) in tokens.iter().enumerate() {
        match *tok {
            TimeRsTok::Space => {
                if !r.peek_is(' ') {
                    return r.fail("expected a space");
                }
                while r.peek_is(' ') {
                    r.pos += 1;
                }
            }
            TimeRsTok::Lit(c) | TimeRsTok::Dir(c @ '%') => {
                if !r.peek_is(c) {
                    return r.fail(&format!("expected {}", time_rs_repr(&c.to_string())));
                }
                r.pos += 1;
            }
            TimeRsTok::ColonZ => {
                let o = time_rs_read_offset(&mut r, ":z")?;
                f.set(TimeRsKey::Offset, TimeRsNum::small(o), ":z")?;
            }
            TimeRsTok::Dir('Y') => {
                let neg = r.sign("-").is_some();
                let most = if time_rs_is_numeric(tokens.get(n + 1)) { Some(4) } else { None };
                let ds = r.digits(most, "Y")?;
                f.set(TimeRsKey::Year, TimeRsNum::parse(neg, &ds), "Y")?;
            }
            TimeRsTok::Dir(v @ ('m' | 'd' | 'H' | 'M' | 'S')) => {
                let key = match v {
                    'm' => TimeRsKey::Month,
                    'd' => TimeRsKey::Day,
                    'H' => TimeRsKey::Hour,
                    'M' => TimeRsKey::Minute,
                    _ => TimeRsKey::Second,
                };
                let d = v.to_string();
                let ds = r.digits(Some(2), &d)?;
                f.set(key, TimeRsNum::parse(false, &ds), &d)?;
            }
            TimeRsTok::Dir('j') => {
                let ds = r.digits(Some(3), "j")?;
                f.set(TimeRsKey::Doy, TimeRsNum::parse(false, &ds), "j")?;
            }
            TimeRsTok::Dir('s') => {
                let neg = r.sign("-").is_some();
                let ds = r.digits(None, "s")?;
                f.set(TimeRsKey::Epoch, TimeRsNum::parse(neg, &ds), "s")?;
            }
            TimeRsTok::Dir('f') => {
                let mut micro = 0;
                if r.peek_is('.') {
                    r.pos += 1;
                    let ds = r.digits(Some(9), "f")?;
                    if r.peek_digit() {
                        return r.fail("more than 9 fraction digits for %f");
                    }
                    let mut six: String = ds.chars().take(6).collect();
                    while six.len() < 6 {
                        six.push('0');
                    }
                    micro = six.parse().unwrap();
                }
                f.set(TimeRsKey::Micro, TimeRsNum::small(micro), "f")?;
            }
            TimeRsTok::Dir('a') => {
                let i = r.name(&TIME_RS_WEEKDAYS, "a")?;
                f.set(TimeRsKey::Weekday, TimeRsNum::small(i as i128), "a")?;
            }
            TimeRsTok::Dir('b') => {
                let i = r.name(&TIME_RS_MONTHS, "b")?;
                f.set(TimeRsKey::Month, TimeRsNum::small(i as i128 + 1), "b")?;
            }
            TimeRsTok::Dir(_) => {
                let o = time_rs_read_offset(&mut r, "z")?;
                f.set(TimeRsKey::Offset, TimeRsNum::small(o), "z")?;
            }
        }
    }
    if r.pos != r.text.len() {
        return r.fail("unexpected text");
    }
    Ok(f)
}

fn time_rs_resolve(f: &TimeRsFields, def_offset: i128, def_year: i128) -> Result<i128, String> {
    let offset = f.val(TimeRsKey::Offset, def_offset).v;
    let micro = f.val(TimeRsKey::Micro, 0).v;
    if let Some((_, secs, _)) = f.get(TimeRsKey::Epoch) {
        let t = secs.v * TIME_RS_US + micro;
        if secs.v.abs() > TIME_RS_BIG || !time_rs_in_i64(t) {
            return Err(format!("%s {} is outside the range of an instant", secs.text));
        }
        let c = time_rs_civil(offset, t);
        for (k, cv) in [
            (TimeRsKey::Year, c.year),
            (TimeRsKey::Month, c.month),
            (TimeRsKey::Day, c.day),
            (TimeRsKey::Hour, c.hour),
            (TimeRsKey::Minute, c.minute),
            (TimeRsKey::Second, c.second),
            (TimeRsKey::Doy, c.doy),
            (TimeRsKey::Weekday, c.weekday),
        ] {
            if let Some((_, v, d)) = f.get(k) {
                if v.text != cv.to_string() {
                    return Err(format!("%{} gives {} but %s {} has {}", d, v.text, secs.text, cv));
                }
            }
        }
        return Ok(t);
    }
    let yn = f.val(TimeRsKey::Year, def_year);
    let y = yn.v;
    let (m, d);
    if let Some((_, doyn, _)) = f.get(TimeRsKey::Doy) {
        let doy = doyn.v;
        let top = if time_rs_is_leap(y) { 366 } else { 365 };
        if !(1 <= doy && doy <= top) {
            return Err(format!("%j {} does not exist in {}", doyn.text, yn.year_text()));
        }
        let (_, mm, dd) = time_rs_civil_from_days(time_rs_days_from_civil(y, 1, 1) + doy - 1);
        for (k, v) in [(TimeRsKey::Month, mm), (TimeRsKey::Day, dd)] {
            if let Some((_, fv, fd)) = f.get(k) {
                if fv.text != v.to_string() {
                    return Err(format!(
                        "%j {} is {}-{}, not %{} {}",
                        doyn.text,
                        time_rs_pad_int(mm, 2),
                        time_rs_pad_int(dd, 2),
                        fd,
                        fv.text
                    ));
                }
            }
        }
        m = mm;
        d = dd;
    } else {
        m = f.val(TimeRsKey::Month, 1).v;
        d = f.val(TimeRsKey::Day, 1).v;
    }
    let hour = f.val(TimeRsKey::Hour, 0).v;
    let minute = f.val(TimeRsKey::Minute, 0).v;
    let second = f.val(TimeRsKey::Second, 0).v;
    if let Some(e) = time_rs_date_error(y, &yn.pad04(), m, d).or_else(|| time_rs_time_error(hour, minute, second, micro)) {
        return Err(e);
    }
    let days = time_rs_days_from_civil(y, m, d);
    if let Some((_, wv, _)) = f.get(TimeRsKey::Weekday) {
        if wv.v != time_rs_weekday_index(days) {
            return Err(format!(
                "%a gives {} but {}-{}-{} is a {}",
                TIME_RS_WEEKDAYS[wv.v as usize],
                yn.year_text(),
                time_rs_pad_int(m, 2),
                time_rs_pad_int(d, 2),
                TIME_RS_WEEKDAYS[time_rs_weekday_index(days) as usize]
            ));
        }
    }
    let us_of_day = ((hour * 60 + minute) * 60 + second) * TIME_RS_US + micro;
    let t = time_rs_instant(days, us_of_day, offset);
    if !time_rs_in_i64(t) {
        return Err(format!(
            "{}-{}-{} is outside the range of an instant",
            yn.year_text(),
            time_rs_pad_int(m, 2),
            time_rs_pad_int(d, 2)
        ));
    }
    Ok(t)
}

pub fn time_rs_parse_time(opts: &TimeParse, text: &String) -> Try<String, i64> {
    let r = time_rs_tokens(&opts.format)
        .and_then(|ts| time_rs_scan(&ts, text))
        .and_then(|f| time_rs_resolve(&f, opts.offset as i128, opts.year as i128));
    match r {
        Ok(t) => time_rs_ok(t as i64),
        Err(e) => time_rs_err(e),
    }
}

// ---------------------------------------------------------------------------
// Durations as text

fn time_rs_show_positive(d: i128) -> String {
    if d < 1000 {
        return format!("{}us", d);
    }
    if d < TIME_RS_US {
        return format!("{}{}ms", d / 1000, time_rs_fraction_text(d % 1000, 3));
    }
    let h = d / TIME_RS_HOUR;
    let rem = d % TIME_RS_HOUR;
    let m = rem / TIME_RS_MINUTE;
    let rem = rem % TIME_RS_MINUTE;
    let (s, frac) = (rem / TIME_RS_US, rem % TIME_RS_US);
    let mut out = String::new();
    if h != 0 {
        out.push_str(&format!("{}h", h));
    }
    if h != 0 || m != 0 {
        out.push_str(&format!("{}m", m));
    }
    out.push_str(&format!("{}{}s", s, time_rs_fraction_text(frac, 6)));
    out
}

pub fn time_rs_show_duration(d: &i64) -> String {
    let d = *d as i128;
    if d == 0 {
        "0s".to_string()
    } else if d < 0 {
        format!("-{}", time_rs_show_positive(-d))
    } else {
        time_rs_show_positive(d)
    }
}

// Exact sum of non-negative terms: a saturating integer part and the
// decimal digits of the fraction, most significant first.
struct TimeRsSum {
    whole: u128,
    frac: Vec<u8>,
}

const TIME_RS_SUM_CAP: u128 = 1 << 100;

impl TimeRsSum {
    fn add_whole(&mut self, x: u128) {
        self.whole = self.whole.saturating_add(x).min(TIME_RS_SUM_CAP);
    }

    // Adds (digits as an integer) * scale / 10^digits.len().
    fn add_scaled_fraction(&mut self, digits: &[u8], scale: u64) {
        let mut prod: Vec<u64> = digits.iter().map(|&d| d as u64 * scale).collect();
        let mut carry: u64 = 0;
        for p in prod.iter_mut().rev() {
            let x = *p + carry;
            *p = x % 10;
            carry = x / 10;
        }
        self.add_whole(carry as u128);
        if self.frac.len() < prod.len() {
            self.frac.resize(prod.len(), 0);
        }
        let mut c: u8 = 0;
        for i in (0..self.frac.len()).rev() {
            let x = self.frac[i] + prod.get(i).copied().unwrap_or(0) as u8 + c;
            self.frac[i] = x % 10;
            c = x / 10;
        }
        self.add_whole(c as u128);
    }
}

pub fn time_rs_read_duration(s: &String) -> Try<String, i64> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut neg = false;
    if let Some(&c) = cs.first() {
        if c == '+' || c == '-' {
            neg = c == '-';
            i = 1;
        }
    }
    if cs.len() == i + 1 && cs[i] == '0' {
        return time_rs_ok(0);
    }
    if i == cs.len() {
        return time_rs_err(format!("expected a duration such as 1h30m or 2.5s, got {}", time_rs_repr(s)));
    }
    let units: [(&str, u64); 5] = [
        ("us", 1),
        ("ms", 1000),
        ("h", TIME_RS_HOUR as u64),
        ("m", TIME_RS_MINUTE as u64),
        ("s", TIME_RS_US as u64),
    ];
    let mut total = TimeRsSum { whole: 0, frac: Vec::new() };
    while i < cs.len() {
        let mut j = i;
        while j < cs.len() && cs[j].is_ascii_digit() {
            j += 1;
        }
        let whole: Vec<u8> = cs[i..j].iter().map(|c| *c as u8 - b'0').collect();
        let mut frac: Vec<u8> = Vec::new();
        if j < cs.len() && cs[j] == '.' {
            let mut k = j + 1;
            while k < cs.len() && cs[k].is_ascii_digit() {
                k += 1;
            }
            frac = cs[j + 1..k].iter().map(|c| *c as u8 - b'0').collect();
            j = k;
        }
        if whole.is_empty() && frac.is_empty() {
            return time_rs_err(format!("expected a number at character {} of {}", i + 1, time_rs_repr(s)));
        }
        let mut unit = None;
        for (name, scale) in units.iter() {
            let nc: Vec<char> = name.chars().collect();
            if cs.len() >= j + nc.len() && cs[j..j + nc.len()] == nc[..] {
                unit = Some((nc.len(), *scale));
                break;
            }
        }
        let (ulen, scale) = match unit {
            Some(u) => u,
            None => {
                return time_rs_err(format!(
                    "expected a unit (h, m, s, ms or us) at character {} of {}",
                    j + 1,
                    time_rs_repr(s)
                ))
            }
        };
        let w = whole.iter().fold(0u128, |acc, &d| (acc * 10 + d as u128).min(TIME_RS_SUM_CAP));
        total.add_whole(w.saturating_mul(scale as u128));
        total.add_scaled_fraction(&frac, scale);
        i = j + ulen;
    }
    let v = total.whole as i128;
    let v = if neg { -v } else { v };
    if !time_rs_in_i64(v) {
        return time_rs_err(format!("duration {} is outside the range of I64 microseconds", time_rs_repr(s)));
    }
    time_rs_ok(v as i64)
}
