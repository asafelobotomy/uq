//! `uq_core::integration::dbus_json` calls on a private bus: every allowed type over the wire,
//! refused types, the deadline, the size limits and `NO_AUTO_START`.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::json;
use uq_core::integration::dbus_json::{
    DbusJsonError, MAX_RESULT_JSON, Target, call, get_properties, get_property,
};
use uq_core::test_bus::{connect, private_bus};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const NAME: &str = "org.uq.Test1";

struct Fake;

type Containers = (
    Vec<String>,
    Vec<i32>,
    HashMap<String, OwnedValue>,
    HashMap<String, String>,
);

#[zbus::interface(name = "org.uq.Test1")]
impl Fake {
    #[allow(clippy::too_many_arguments)]
    fn basics(
        &self,
        b: bool,
        y: u8,
        n: i16,
        q: u16,
        i: i32,
        u: u32,
        x: i64,
        t: u64,
        d: f64,
        s: String,
        o: OwnedObjectPath,
    ) -> (
        bool,
        u8,
        i16,
        u16,
        i32,
        u32,
        i64,
        u64,
        f64,
        String,
        OwnedObjectPath,
    ) {
        (b, y, n, q, i, u, x, t, d, s, o)
    }

    fn containers(
        &self,
        a: Vec<String>,
        ai: Vec<i32>,
        sv: HashMap<String, OwnedValue>,
        ss: HashMap<String, String>,
    ) -> Containers {
        (a, ai, sv, ss)
    }

    fn one(&self, s: String) -> String {
        format!("got {s}")
    }

    fn nothing(&self) {}

    fn pair(&self) -> (i32, (i32, String)) {
        (1, (2, "x".into()))
    }

    fn takes_struct(&self, _s: (i32, String)) {}

    async fn slow(&self) {
        tokio::time::sleep(Duration::from_secs(4)).await;
    }

    fn big(&self) -> String {
        "x".repeat(MAX_RESULT_JSON + 10)
    }

    #[zbus(property)]
    fn count(&self) -> u32 {
        7
    }

    #[zbus(property)]
    fn label(&self) -> String {
        "seven".into()
    }

    #[zbus(property)]
    fn pairs(&self) -> Vec<(i32, i32)> {
        vec![(1, 2)]
    }
}

fn target() -> Target {
    Target {
        name: NAME.into(),
        path: "/org/uq/Test1".into(),
        interface: NAME.into(),
    }
}

async fn serve(bus: &uq_core::test_bus::Bus) -> zbus::Connection {
    let conn = connect(bus).await;
    conn.object_server()
        .at("/org/uq/Test1", Fake)
        .await
        .unwrap();
    conn.request_name(NAME).await.unwrap();
    conn
}

#[tokio::test]
async fn every_allowed_type_crosses_the_bus() {
    let bus = private_bus();
    let _svc = serve(&bus).await;
    let c = connect(&bus).await;
    let t = target();

    let basics = json!([
        true,
        255,
        -2,
        65535,
        -5,
        7,
        i64::MIN,
        u64::MAX,
        0.5,
        "hé",
        "/a/b"
    ]);
    assert_eq!(
        call(&c, &t, "Basics", &basics, false).await.unwrap(),
        basics
    );

    let containers = json!([["a", "b"], [1, -2], {"on": true, "n": 3, "s": "x", "l": ["q"], "m": {"k": 1.5}}, {"k": "v"}]);
    assert_eq!(
        call(&c, &t, "Containers", &containers, false)
            .await
            .unwrap(),
        containers
    );

    assert_eq!(
        call(&c, &t, "One", &json!(["x"]), false).await.unwrap(),
        json!("got x")
    );
    assert_eq!(
        call(&c, &t, "Nothing", &json!([]), false).await.unwrap(),
        json!(null)
    );
    assert_eq!(
        call(&c, &t, "Nothing", &json!(null), false).await.unwrap(),
        json!(null)
    );

    assert_eq!(
        get_property(&c, &t, "Count", false).await.unwrap(),
        json!(7)
    );
    assert_eq!(
        get_properties(
            &c,
            &t,
            &["Count".into(), "Label".into(), "Missing".into()],
            false
        )
        .await
        .unwrap(),
        json!({"Count": 7, "Label": "seven"})
    );
}

#[tokio::test]
async fn refused_types_bad_args_and_limits() {
    let bus = private_bus();
    let _svc = serve(&bus).await;
    let c = connect(&bus).await;
    let t = target();

    let unsupported = |r: Result<serde_json::Value, DbusJsonError>| {
        matches!(r, Err(DbusJsonError::Unsupported(_)))
    };
    assert!(unsupported(call(&c, &t, "Pair", &json!([]), false).await));
    assert!(unsupported(
        call(&c, &t, "TakesStruct", &json!([[1, "x"]]), false).await
    ));
    assert!(unsupported(get_property(&c, &t, "Pairs", false).await));
    assert!(matches!(
        call(&c, &t, "One", &json!([1]), false).await,
        Err(DbusJsonError::BadValue(_))
    ));
    assert!(matches!(
        call(&c, &t, "One", &json!(["a", "b"]), false).await,
        Err(DbusJsonError::BadValue(_))
    ));
    let huge = json!(["x".repeat(17 * 1024)]);
    assert_eq!(
        call(&c, &t, "One", &huge, false).await,
        Err(DbusJsonError::ArgsTooLarge)
    );
    assert_eq!(
        call(&c, &t, "Big", &json!([]), false).await,
        Err(DbusJsonError::ResultTooLarge)
    );
    assert!(matches!(
        call(&c, &t, "NoSuchMethod", &json!([]), false).await,
        Err(DbusJsonError::Call(_))
    ));
    assert!(matches!(
        call(&c, &t, "bad member!", &json!([]), false).await,
        Err(DbusJsonError::NotAllowed(_))
    ));

    let start = std::time::Instant::now();
    assert_eq!(
        call(&c, &t, "Slow", &json!([]), false).await,
        Err(DbusJsonError::Deadline)
    );
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn no_auto_start_needs_an_owner() {
    let bus = private_bus();
    let c = connect(&bus).await;
    assert_eq!(
        call(&c, &target(), "Nothing", &json!([]), false).await,
        Err(DbusJsonError::NotRunning(NAME.into()))
    );
    assert_eq!(
        get_property(&c, &target(), "Count", false).await,
        Err(DbusJsonError::NotRunning(NAME.into()))
    );
    // With auto start allowed, the bus itself answers that nothing provides the name.
    assert!(matches!(
        call(&c, &target(), "Nothing", &json!([]), true).await,
        Err(DbusJsonError::Call(_))
    ));
}
