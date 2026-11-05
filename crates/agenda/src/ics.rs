//! iCalendar (.ics): os eventos (VEVENT) com título, início, fim, dia inteiro
//! e notas. Horários em UTC ("Z") viram hora local; com TZID, ficam como estão.

/// (ano, mês, dia, hora, minuto).
pub type Stamp = (i32, u32, u32, u32, u32);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Event {
    pub uid: String,
    pub title: String,
    pub start: Stamp,
    pub end: Stamp,
    pub all_day: bool,
    pub notes: String,
}

fn unescape(v: &str) -> String {
    v.replace("\\n", "\n").replace("\\N", "\n").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\")
}

fn escape(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\n', "\\n").replace(',', "\\,").replace(';', "\\;")
}

/// UTC → hora local (pelo libc).
fn utc_to_local(s: Stamp) -> Stamp {
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = s.0 - 1900;
        tm.tm_mon = s.1 as i32 - 1;
        tm.tm_mday = s.2 as i32;
        tm.tm_hour = s.3 as i32;
        tm.tm_min = s.4 as i32;
        let t = libc::timegm(&mut tm);
        let mut out: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut out);
        (out.tm_year + 1900, out.tm_mon as u32 + 1, out.tm_mday as u32, out.tm_hour as u32, out.tm_min as u32)
    }
}

/// "20261105" ou "20261105T143000[Z]" → (data e hora, só data?).
pub fn parse_stamp(v: &str) -> Option<(Stamp, bool)> {
    let v = v.trim();
    let num = |a: usize, b: usize| v.get(a..b)?.parse::<u32>().ok();
    let (y, m, d) = (num(0, 4)? as i32, num(4, 6)?, num(6, 8)?);
    if v.len() == 8 {
        return Some(((y, m, d, 0, 0), true));
    }
    let (h, mi) = (num(9, 11)?, num(11, 13)?);
    let s = (y, m, d, h, mi);
    Some((if v.ends_with('Z') { utc_to_local(s) } else { s }, false))
}

fn fmt_stamp(s: Stamp, date_only: bool) -> String {
    if date_only { format!("{:04}{:02}{:02}", s.0, s.1, s.2) } else { format!("{:04}{:02}{:02}T{:02}{:02}00", s.0, s.1, s.2, s.3, s.4) }
}

pub fn parse(text: &str) -> Vec<Event> {
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.trim_end_matches('\r');
        if (l.starts_with(' ') || l.starts_with('\t')) && !lines.is_empty() {
            lines.last_mut().unwrap().push_str(&l[1..]);
        } else {
            lines.push(l.to_string());
        }
    }
    let mut out = Vec::new();
    let mut cur: Option<Event> = None;
    let mut has_end = false;
    for l in lines {
        let Some((key, value)) = l.split_once(':') else { continue };
        let name = key.split(';').next().unwrap_or(key).to_ascii_uppercase();
        match (name.as_str(), cur.as_mut()) {
            ("BEGIN", _) if value == "VEVENT" => {
                cur = Some(Event::default());
                has_end = false;
            }
            ("END", Some(_)) if value == "VEVENT" => {
                let mut e = cur.take().unwrap();
                if !has_end {
                    e.end = e.start;
                }
                out.push(e);
            }
            ("SUMMARY", Some(e)) => e.title = unescape(value),
            ("DESCRIPTION", Some(e)) => e.notes = unescape(value),
            ("UID", Some(e)) => e.uid = value.to_string(),
            ("DTSTART", Some(e)) => {
                if let Some((s, d)) = parse_stamp(value) {
                    e.start = s;
                    e.all_day = d;
                }
            }
            ("DTEND", Some(e)) => {
                if let Some((s, _)) = parse_stamp(value) {
                    e.end = s;
                    has_end = true;
                }
            }
            _ => {}
        }
    }
    out
}

pub fn serialize(events: &[Event]) -> String {
    let mut s = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Glove//Agenda//PT-BR\r\n");
    for e in events {
        s.push_str("BEGIN:VEVENT\r\n");
        s.push_str(&format!("UID:{}\r\n", e.uid));
        s.push_str(&format!("SUMMARY:{}\r\n", escape(&e.title)));
        if e.all_day {
            s.push_str(&format!("DTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\n", fmt_stamp(e.start, true), fmt_stamp(e.end, true)));
        } else {
            s.push_str(&format!("DTSTART:{}\r\nDTEND:{}\r\n", fmt_stamp(e.start, false), fmt_stamp(e.end, false)));
        }
        if !e.notes.is_empty() {
            s.push_str(&format!("DESCRIPTION:{}\r\n", escape(&e.notes)));
        }
        s.push_str("END:VEVENT\r\n");
    }
    s.push_str("END:VCALENDAR\r\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let ev = vec![
            Event { uid: "a".into(), title: "Dentista, 2º andar".into(), start: (2026, 11, 5, 14, 30), end: (2026, 11, 5, 15, 0), all_day: false, notes: "levar\ncarteirinha".into() },
            Event { uid: "b".into(), title: "Feriado".into(), start: (2026, 11, 15, 0, 0), end: (2026, 11, 16, 0, 0), all_day: true, notes: String::new() },
        ];
        assert_eq!(parse(&serialize(&ev)), ev);
    }

    #[test]
    fn horarios() {
        assert_eq!(parse_stamp("20261105"), Some(((2026, 11, 5, 0, 0), true)));
        assert_eq!(parse_stamp("20261105T143000"), Some(((2026, 11, 5, 14, 30), false)));
        assert!(parse_stamp("2026").is_none());
    }
}
