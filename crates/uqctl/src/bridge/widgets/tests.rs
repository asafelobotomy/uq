use super::*;

#[test]
fn file_urls_are_encoded() {
    assert_eq!(
        file_url(Path::new("/usr/share/uq/shell/widgets/a-b/W idget.qml")),
        "file:///usr/share/uq/shell/widgets/a-b/W%20idget.qml"
    );
    assert_eq!(file_url(Path::new("/x/ü.qml")), "file:///x/%C3%BC.qml");
}

fn known(id: &str, order: u8, bus_name: Option<&str>, bins_ok: bool) -> Known {
    let toml = format!(
        "uq_api = 1\nid = \"{id}\"\nname = \"N {id}\"\nslot = \"bar\"\nqml = \"W.qml\"\n\
         order = {order}\n{}",
        bus_name.map_or(String::new(), |n| format!(
            "[requires]\nbus_name = \"{n}\"\n"
        ))
    );
    Known {
        manifest: uq_core::integration::manifest::parse_widget(&toml).unwrap(),
        qml_url: format!("file:///w/{id}/W.qml"),
        rev: format!("rev-{id}"),
        bins_ok,
    }
}

#[test]
fn rev_follows_file_changes() {
    let dir = std::env::temp_dir().join(format!("uq-rev-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("W.qml"), "Item {}").unwrap();
    std::fs::write(dir.join("sub/x.js"), "1").unwrap();
    let first = content_rev(&dir);
    assert_eq!(content_rev(&dir), first, "same files, same rev");
    std::fs::write(dir.join("sub/x.js"), "22").unwrap();
    let second = content_rev(&dir);
    assert_ne!(second, first, "a changed file beside the QML changes it");
    std::fs::write(dir.join("Details.qml"), "Item {}").unwrap();
    assert_ne!(content_rev(&dir), second, "a new file changes it");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn list_orders_and_gates() {
    let mut w = BTreeMap::new();
    w.insert("b".into(), known("b", 10, None, true));
    w.insert("a".into(), known("a", 10, Some("org.x.A"), true));
    w.insert("c".into(), known("c", 5, None, false));
    let mut owners = HashMap::new();
    owners.insert((Bus::Session, "org.x.A".to_string()), None);
    let l = list_json(&w, &owners);
    let ids: Vec<&str> = l
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["c", "a", "b"], "order, then id");
    assert_eq!(l[0]["visible"], false, "a missing program hides it");
    assert_eq!(l[1]["visible"], false, "no owner yet");
    assert_eq!(l[2]["visible"], true);
    assert_eq!(
        l[2],
        json!({"id":"b","name":"N b","slot":"bar","order":10,
               "qml_url":"file:///w/b/W.qml","rev":"rev-b","visible":true})
    );
    owners.insert((Bus::Session, "org.x.A".to_string()), Some(":1.5".into()));
    assert_eq!(list_json(&w, &owners)[1]["visible"], true);
}
