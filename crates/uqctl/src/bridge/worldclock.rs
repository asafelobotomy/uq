//! The world clock island's zones: Qt's JavaScript has no time-zone support, so the bridge
//! sends each zone's current UTC offset. Sent when placed, when the zones change, and on every
//! 5-minute boundary of the wall clock, so a DST change lands within the minute it happens.
//! Only while `worldclock` is placed; no network.

use super::feeds::placed;
use super::protocol::Outbound;
use chrono::{DateTime, Offset, TimeZone, Utc};
use chrono_tz::{OffsetName, Tz};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use uq_core::settings::Settings;

/// Seconds between re-sends, aligned to the wall clock.
const STEP: i64 = 300;

/// `America/New_York` → `New York`; `UTC` stays.
pub fn label(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

/// A zone by its IANA name, in any letter case (`europe/paris` is `Europe/Paris`).
fn find_zone(z: &str) -> Option<Tz> {
    z.parse().ok().or_else(|| {
        chrono_tz::TZ_VARIANTS
            .iter()
            .copied()
            .find(|tz| tz.name().eq_ignore_ascii_case(z))
    })
}

/// The payload for `zones` at `now`; unknown zone names are left out. `zone` is the name as
/// the user saved it; `label` comes from the zone's proper name.
pub fn payload(zones: &[String], now: DateTime<Utc>) -> Value {
    json!(
        zones
            .iter()
            .filter_map(|z| {
                let tz = find_zone(z)?;
                let off = tz.offset_from_utc_datetime(&now.naive_utc());
                Some(json!({
                    "zone": z,
                    "label": label(tz.name()),
                    "offset_s": off.fix().local_minus_utc(),
                    "abbrev": off.abbreviation(),
                }))
            })
            .collect::<Vec<_>>()
    )
}

/// Time until the next multiple of `STEP` seconds of the wall clock.
fn until_next_step(now: DateTime<Utc>) -> Duration {
    let into = now.timestamp().rem_euclid(STEP);
    let ms = (STEP - into) * 1000 - i64::from(now.timestamp_subsec_millis());
    Duration::from_millis(ms.clamp(1, STEP * 1000) as u64)
}

fn zones(s: &Settings) -> Option<Vec<String>> {
    placed(s, "worldclock").then(|| {
        s.shell
            .bar
            .as_ref()
            .map(|b| b.options.worldclock.zones.clone())
            .unwrap_or_default()
    })
}

pub async fn run(
    tx: mpsc::Sender<Outbound>,
    mut rx: watch::Receiver<Option<Settings>>,
    stop: CancellationToken,
) {
    let emit = |data: Value| {
        let (tx, stop) = (tx.clone(), stop.clone());
        async move {
            let out = Outbound::Event {
                event: "WorldClock".into(),
                data,
            };
            tokio::select! { _ = stop.cancelled() => false, r = tx.send(out) => r.is_ok() }
        }
    };
    let mut current: Option<Vec<String>> = None;
    loop {
        let want = rx.borrow_and_update().as_ref().and_then(zones);
        if want != current {
            let had = current.is_some();
            current = want;
            let data = match &current {
                Some(z) => payload(z, Utc::now()),
                None => Value::Null,
            };
            if (current.is_some() || had) && !emit(data).await {
                return;
            }
        }
        let wait = until_next_step(Utc::now());
        tokio::select! {
            _ = stop.cancelled() => return,
            r = rx.changed() => if r.is_err() { return },
            _ = tokio::time::sleep(wait), if current.is_some() => {
                if let Some(z) = &current
                    && !emit(payload(z, Utc::now())).await
                {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uq_core::settings::BarItem;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn offsets_for_a_fixed_instant() {
        let z: Vec<String> = ["America/New_York", "Asia/Kolkata", "UTC", "Nowhere/Land"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            payload(&z, at("2026-07-01T12:00:00Z")),
            json!([
                {"zone": "America/New_York", "label": "New York", "offset_s": -4 * 3600, "abbrev": "EDT"},
                {"zone": "Asia/Kolkata", "label": "Kolkata", "offset_s": 19800, "abbrev": "IST"},
                {"zone": "UTC", "label": "UTC", "offset_s": 0, "abbrev": "UTC"},
            ])
        );
        // DST ends 2026-11-01 06:00 UTC in New York.
        let before = payload(&z[..1], at("2026-11-01T05:59:00Z"));
        let after = payload(&z[..1], at("2026-11-01T06:00:00Z"));
        assert_eq!(before[0]["offset_s"], -4 * 3600);
        assert_eq!(after[0]["offset_s"], -5 * 3600);
        assert_eq!(after[0]["abbrev"], "EST");
        assert_eq!(label("America/Argentina/Buenos_Aires"), "Buenos Aires");
        // Any letter case finds the zone; the saved spelling is kept for the shell to match.
        let lower = payload(&["europe/paris".to_string()], at("2026-07-01T12:00:00Z"));
        assert_eq!(
            lower,
            json!([{"zone": "europe/paris", "label": "Paris", "offset_s": 7200, "abbrev": "CEST"}])
        );
    }

    #[test]
    fn steps_align_to_five_minutes() {
        assert_eq!(
            until_next_step(at("2026-11-13T10:03:00Z")),
            Duration::from_secs(120)
        );
        assert_eq!(
            until_next_step(at("2026-11-13T10:05:00Z")),
            Duration::from_secs(300)
        );
        assert_eq!(
            until_next_step(at("2026-11-13T10:04:59.500Z")),
            Duration::from_millis(500)
        );
    }

    #[tokio::test]
    async fn sends_while_placed_and_null_on_removal() {
        let mut on = Settings::default();
        let bar = on.shell.bar.as_mut().unwrap();
        bar.end.push(BarItem::new("worldclock"));
        bar.options.worldclock.zones = vec!["Europe/Paris".into()];
        let (stx, srx) = watch::channel(Some(Settings::default()));
        let (tx, mut rx) = mpsc::channel(8);
        let stop = CancellationToken::new();
        let task = tokio::spawn(run(tx, srx, stop.clone()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err());
        stx.send_replace(Some(on.clone()));
        let next = |rx: &mut mpsc::Receiver<Outbound>| {
            let got = rx.try_recv();
            match got {
                Ok(Outbound::Event { event, data }) => (event, data),
                _ => panic!("expected event"),
            }
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
        let (e, d) = next(&mut rx);
        assert_eq!(e, "WorldClock");
        assert_eq!(d[0]["label"], "Paris");
        // New zones: sent at once.
        on.shell.bar.as_mut().unwrap().options.worldclock.zones = vec!["Asia/Tokyo".into()];
        stx.send_replace(Some(on));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(next(&mut rx).1[0]["offset_s"], 9 * 3600);
        stx.send_replace(Some(Settings::default()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(next(&mut rx).1, Value::Null);
        stop.cancel();
        task.await.unwrap();
    }
}
