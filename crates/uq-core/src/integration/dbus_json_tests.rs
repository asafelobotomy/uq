use super::*;
use serde_json::json;

#[test]
fn signatures_split_into_allowed_types() {
    assert_eq!(
        parse_signature("bsa{sv}ata{ss}o").unwrap(),
        [
            Ty::Basic('b'),
            Ty::Basic('s'),
            Ty::DictSv,
            Ty::Array('t'),
            Ty::DictSs,
            Ty::Basic('o'),
        ]
    );
    assert_eq!(parse_signature("").unwrap(), []);
    for bad in [
        "v", "g", "h", "(is)", "a(is)", "aas", "a{si}", "a{sv", "aa{sv}", "a", "a{ii}", "av", "s(",
    ] {
        assert!(
            matches!(parse_signature(bad), Err(DbusJsonError::Unsupported(_))),
            "{bad}"
        );
    }
}

fn round_trip(sig: &str, v: Json) -> Json {
    let ty = parse_signature(sig).unwrap()[0];
    let value = json_to_value(ty, &v).unwrap();
    assert_eq!(value.value_signature().to_string(), sig, "{v}");
    value_to_json(&value).unwrap()
}

#[test]
fn every_allowed_type_round_trips() {
    for (sig, v) in [
        ("b", json!(true)),
        ("y", json!(255)),
        ("n", json!(-32768)),
        ("q", json!(65535)),
        ("i", json!(-2147483648i64)),
        ("u", json!(4294967295u64)),
        ("x", json!(i64::MIN)),
        ("t", json!(u64::MAX)),
        ("d", json!(1.5)),
        ("s", json!("héllo \"quoted\"")),
        ("o", json!("/org/uq/Test1")),
        ("as", json!(["a", "b"])),
        ("ab", json!([true, false])),
        ("ay", json!([0, 255])),
        ("an", json!([-1])),
        ("aq", json!([1])),
        ("ai", json!([1, -2])),
        ("au", json!([3])),
        ("ax", json!([-4])),
        ("at", json!([5])),
        ("ad", json!([0.25])),
        ("ao", json!(["/a", "/b/c"])),
        ("as", json!([])),
        ("a{ss}", json!({"k": "v"})),
        (
            "a{sv}",
            json!({"b": true, "i": -3, "big": u64::MAX, "f": 2.5, "s": "x",
                   "list": ["a"], "nums": [1, 2.5], "bools": [false], "empty": [],
                   "nested": {"deeper": {"n": 1}}}),
        ),
    ] {
        let back = round_trip(sig, v.clone());
        if sig == "a{sv}" {
            // Whole numbers in a mixed list come back as doubles.
            let mut want = v.clone();
            want["nums"] = json!([1.0, 2.5]);
            assert_eq!(back, want, "{sig}");
        } else {
            assert_eq!(back, v, "{sig}");
        }
    }
    // A double that holds a whole number stays a number.
    assert_eq!(round_trip("d", json!(2)), json!(2.0));
}

#[test]
fn values_are_checked_against_the_type() {
    for (sig, v) in [
        ("b", json!(1)),
        ("y", json!(256)),
        ("y", json!(-1)),
        ("n", json!(40000)),
        ("q", json!(-1)),
        ("i", json!(1.5)),
        ("u", json!("1")),
        ("x", json!(u64::MAX)),
        ("t", json!(-1)),
        ("d", json!("1.0")),
        ("s", json!(1)),
        ("s", json!("nul\u{0}")),
        ("o", json!("not/a/path")),
        ("o", json!("/trailing/")),
        ("as", json!("x")),
        ("as", json!([1])),
        ("ai", json!([1, "2"])),
        ("a{ss}", json!({"k": 1})),
        ("a{ss}", json!(["k"])),
        ("a{sv}", json!({"n": null})),
        ("a{sv}", json!({"mixed": [1, "a"]})),
        ("a{sv}", json!({"objs": [{}]})),
        ("a{sv}", json!({"a": {"b": {"c": {"d": {"e": 1}}}}})),
    ] {
        let ty = parse_signature(sig).unwrap()[0];
        assert!(
            matches!(json_to_value(ty, &v), Err(DbusJsonError::BadValue(_))),
            "{sig} {v}"
        );
    }
}

#[test]
fn args_match_the_signature_count() {
    assert_eq!(json_to_args("", &json!([])).unwrap().len(), 0);
    assert_eq!(json_to_args("", &Json::Null).unwrap().len(), 0);
    assert_eq!(json_to_args("su", &json!(["a", 1])).unwrap().len(), 2);
    assert!(json_to_args("su", &json!(["a"])).is_err());
    assert!(json_to_args("s", &json!(["a", "b"])).is_err());
    assert!(json_to_args("s", &json!({"a": 1})).is_err());
    assert!(matches!(
        json_to_args("(s)", &json!([["a"]])),
        Err(DbusJsonError::Unsupported(_))
    ));
}

#[test]
fn other_types_are_refused_on_the_way_out() {
    use zbus::zvariant::{Signature, StructureBuilder};
    let st = StructureBuilder::new()
        .add_field(1i32)
        .add_field("x")
        .build()
        .unwrap();
    assert!(matches!(
        value_to_json(&Value::Structure(st)),
        Err(DbusJsonError::Unsupported(_))
    ));
    assert!(value_to_json(&Value::Value(Box::new(Value::U8(1)))).is_err());
    assert!(value_to_json(&Value::Signature(Signature::Str)).is_err());
    assert!(value_to_json(&Value::F64(f64::NAN)).is_err());
    let mut nested = Array::new(&Signature::try_from("as").unwrap());
    nested.append(Value::from(vec!["a".to_string()])).unwrap();
    assert!(value_to_json(&Value::Array(nested)).is_err(), "aas");
    let mut d = Dict::new(&Signature::Str, &Signature::I32);
    d.append(Value::from("k"), Value::I32(1)).unwrap();
    assert!(value_to_json(&Value::Dict(d)).is_err(), "a{{si}}");
}

const XML: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.other.Iface">
    <method name="Add"><arg type="i" direction="in"/></method>
  </interface>
  <interface name="org.uq.Test1">
    <method name="Ping"/>
    <method name="Add">
      <arg name="a" type="s" direction="in"/>
      <arg name="b" type="a{sv}"/>
      <arg name="r" type="u" direction="out"/>
    </method>
    <signal name="Done"><arg type="s"/></signal>
  </interface>
</node>"#;

#[test]
fn introspection_gives_the_input_signature() {
    assert_eq!(
        method_in_signature(XML, "org.uq.Test1", "Add").unwrap(),
        "sa{sv}"
    );
    assert_eq!(
        method_in_signature(XML, "org.uq.Test1", "Ping").unwrap(),
        ""
    );
    assert_eq!(
        method_in_signature(XML, "org.other.Iface", "Add").unwrap(),
        "i"
    );
    assert!(method_in_signature(XML, "org.uq.Test1", "Done").is_err());
    assert!(method_in_signature(XML, "org.uq.Nope", "Add").is_err());
    assert!(method_in_signature("<node><interface", "a", "b").is_err());
}
