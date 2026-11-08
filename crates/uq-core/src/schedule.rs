//! Dark/light schedule: off, fixed local times, or sunrise/sunset at a location.

use crate::theme::Variant;
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use sunrise::{Coordinates, SolarDay, SolarEvent};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Schedule {
    #[default]
    Off,
    /// Light from `light_at` until `dark_at` (local time); the window may wrap midnight.
    Fixed {
        light_at: NaiveTime,
        dark_at: NaiveTime,
    },
    /// Light from sunrise to sunset at this location.
    Sun { lat: f64, lon: f64 },
}

impl Schedule {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Schedule::Off => Ok(()),
            Schedule::Fixed { light_at, dark_at } if light_at == dark_at => {
                Err("light and dark times must differ".into())
            }
            Schedule::Fixed { .. } => Ok(()),
            Schedule::Sun { lat, lon } => Coordinates::new(*lat, *lon)
                .map(|_| ())
                .ok_or_else(|| format!("invalid coordinates {lat}, {lon}")),
        }
    }
}

/// Resolve a local wall-clock time. A time inside a DST gap moves to the first instant that
/// exists; an ambiguous time (DST fold) takes the earlier instant.
fn resolve<Tz: TimeZone>(tz: &Tz, date: NaiveDate, t: NaiveTime) -> Option<DateTime<Tz>> {
    let mut ndt = date.and_time(t);
    for _ in 0..=180 {
        match tz.from_local_datetime(&ndt) {
            LocalResult::Single(d) => return Some(d),
            LocalResult::Ambiguous(earlier, _) => return Some(earlier),
            LocalResult::None => ndt += Duration::minutes(1),
        }
    }
    None
}

/// Transitions from the day before `now` to two days after, sorted by time.
fn events<Tz: TimeZone>(s: &Schedule, now: &DateTime<Tz>) -> Vec<(DateTime<Tz>, Variant)> {
    let tz = now.timezone();
    let today = now.date_naive();
    let mut out = Vec::new();
    for offset in -1..=2 {
        let date = today + Duration::days(offset);
        match s {
            Schedule::Off => {}
            Schedule::Fixed { light_at, dark_at } => {
                out.extend(resolve(&tz, date, *light_at).map(|d| (d, Variant::Light)));
                out.extend(resolve(&tz, date, *dark_at).map(|d| (d, Variant::Dark)));
            }
            Schedule::Sun { lat, lon } => {
                let Some(coords) = Coordinates::new(*lat, *lon) else {
                    continue;
                };
                let day = SolarDay::new(coords, date);
                for (event, v) in [
                    (SolarEvent::Sunrise, Variant::Light),
                    (SolarEvent::Sunset, Variant::Dark),
                ] {
                    out.extend(day.event_time(event).map(|d| (d.with_timezone(&tz), v)));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Polar day or night: no sunrise/sunset nearby, so the season decides.
fn polar_variant(lat: f64, month: u32) -> Variant {
    let northern_summer = (4..=9).contains(&month);
    if (lat >= 0.0) == northern_summer {
        Variant::Light
    } else {
        Variant::Dark
    }
}

/// The variant the schedule wants at `now`; `None` when the schedule is off.
pub fn variant_at<Tz: TimeZone>(s: &Schedule, now: &DateTime<Tz>) -> Option<Variant> {
    if *s == Schedule::Off {
        return None;
    }
    let last = events(s, now)
        .into_iter()
        .rev()
        .find(|(at, _)| at <= now)
        .map(|(_, v)| v);
    match (last, s) {
        (Some(v), _) => Some(v),
        (None, Schedule::Sun { lat, .. }) => Some(polar_variant(*lat, now.month())),
        (None, _) => None,
    }
}

/// The most recent scheduled transition at or before `now` (within the last day or so).
pub fn last_event<Tz: TimeZone>(
    s: &Schedule,
    now: &DateTime<Tz>,
) -> Option<(DateTime<Tz>, Variant)> {
    events(s, now).into_iter().rev().find(|(at, _)| at <= now)
}

/// The next time the schedule changes variant, and the variant it changes to.
pub fn next_transition<Tz: TimeZone>(
    s: &Schedule,
    now: &DateTime<Tz>,
) -> Option<(DateTime<Tz>, Variant)> {
    let current = variant_at(s, now)?;
    events(s, now)
        .into_iter()
        .find(|(at, v)| at > now && *v != current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, TimeZone, Utc};
    use chrono_tz::Europe::London;

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn fixed() -> Schedule {
        Schedule::Fixed {
            light_at: hm(7, 0),
            dark_at: hm(19, 0),
        }
    }

    #[test]
    fn off_has_no_variant_or_transition() {
        let now = Utc.with_ymd_and_hms(2027, 1, 1, 12, 0, 0).unwrap();
        assert_eq!(variant_at(&Schedule::Off, &now), None);
        assert_eq!(next_transition(&Schedule::Off, &now), None);
    }

    #[test]
    fn fixed_before_light_is_dark() {
        let now = London.with_ymd_and_hms(2027, 1, 10, 6, 59, 0).unwrap();
        assert_eq!(variant_at(&fixed(), &now), Some(Variant::Dark));
        let noon = London.with_ymd_and_hms(2027, 1, 10, 12, 0, 0).unwrap();
        assert_eq!(variant_at(&fixed(), &noon), Some(Variant::Light));
    }

    #[test]
    fn fixed_next_transition_wraps_midnight() {
        let now = London.with_ymd_and_hms(2027, 1, 10, 20, 0, 0).unwrap();
        let (at, v) = next_transition(&fixed(), &now).unwrap();
        assert_eq!(at, London.with_ymd_and_hms(2027, 1, 11, 7, 0, 0).unwrap());
        assert_eq!(v, Variant::Light);
    }

    #[test]
    fn fixed_inverted_window_wraps() {
        // Light overnight (e.g. a night-shift user): light 22:00, dark 06:00.
        let s = Schedule::Fixed {
            light_at: hm(22, 0),
            dark_at: hm(6, 0),
        };
        let late = London.with_ymd_and_hms(2027, 1, 10, 23, 0, 0).unwrap();
        assert_eq!(variant_at(&s, &late), Some(Variant::Light));
        let noon = London.with_ymd_and_hms(2027, 1, 10, 12, 0, 0).unwrap();
        assert_eq!(variant_at(&s, &noon), Some(Variant::Dark));
    }

    #[test]
    fn fixed_handles_dst_gap() {
        // 2027-03-28: London clocks jump 01:00 GMT -> 02:00 BST, so 01:30 does not exist.
        let s = Schedule::Fixed {
            light_at: hm(1, 30),
            dark_at: hm(19, 0),
        };
        let now = London.with_ymd_and_hms(2027, 3, 28, 0, 30, 0).unwrap();
        let (at, v) = next_transition(&s, &now).unwrap();
        assert_eq!(v, Variant::Light);
        assert_eq!(
            at.with_timezone(&Utc),
            Utc.with_ymd_and_hms(2027, 3, 28, 1, 0, 0).unwrap()
        );
    }

    #[test]
    fn sun_london_midsummer() {
        let s = Schedule::Sun {
            lat: 51.5,
            lon: -0.13,
        };
        let noon = London.with_ymd_and_hms(2027, 6, 21, 12, 0, 0).unwrap();
        assert_eq!(variant_at(&s, &noon), Some(Variant::Light));
        let (at, v) = next_transition(&s, &noon).unwrap();
        assert_eq!(v, Variant::Dark);
        let expected = London.with_ymd_and_hms(2027, 6, 21, 21, 21, 0).unwrap();
        assert!((at - expected).num_minutes().abs() <= 5, "sunset at {at}");
    }

    #[test]
    fn sun_polar_day_is_constant_light() {
        // Svalbard (78°N) in midsummer: the sun never sets.
        let s = Schedule::Sun {
            lat: 78.2,
            lon: 15.6,
        };
        let now = Utc.with_ymd_and_hms(2027, 6, 21, 0, 0, 0).unwrap();
        assert_eq!(variant_at(&s, &now), Some(Variant::Light));
        assert_eq!(next_transition(&s, &now), None);
    }

    #[test]
    fn schedule_json_forms() {
        let s: Schedule =
            serde_json::from_str(r#"{"mode":"fixed","light_at":"07:00:00","dark_at":"19:30:00"}"#)
                .unwrap();
        assert_eq!(
            s,
            Schedule::Fixed {
                light_at: hm(7, 0),
                dark_at: hm(19, 30)
            }
        );
        let s: Schedule = serde_json::from_str(r#"{"mode":"sun","lat":51.5,"lon":-0.13}"#).unwrap();
        assert!(matches!(s, Schedule::Sun { .. }));
        assert_eq!(
            serde_json::from_str::<Schedule>(r#"{"mode":"off"}"#).unwrap(),
            Schedule::Off
        );
    }
}
