use super::*;

#[test]
fn request_layout() {
    let mut r = Request::new(op::CUPS_GET_DEFAULT);
    r.op_attrs(
        "requested-attributes",
        vec![
            Out::Str(tag::KEYWORD, "printer-name".into()),
            Out::Str(tag::KEYWORD, "printer-info".into()),
        ],
    );
    let b = r.encode();
    assert_eq!(&b[..8], &[2, 0, 0x40, 0x01, 0, 0, 0, 1]);
    assert_eq!(b[8], tag::OPERATION);
    assert_eq!(b[9], tag::CHARSET);
    assert_eq!(*b.last().unwrap(), tag::END);
    // The second keyword has no name of its own.
    let tail = &b[b.len() - 1 - (1 + 2 + 2 + "printer-info".len())..b.len() - 1];
    assert_eq!(tail[0], tag::KEYWORD);
    assert_eq!(&tail[1..3], &[0, 0]);
}

#[test]
fn round_trip_through_decode() {
    let mut r = Request::new(op::CUPS_ADD_MODIFY_PRINTER);
    r.op_attr(
        "printer-uri",
        Out::Str(tag::URI, "ipp://localhost/printers/Hall".into()),
    );
    r.printer_attr("printer-info", Out::Str(tag::TEXT, "Hall laser".into()))
        .printer_attr("printer-is-accepting-jobs", Out::Bool(true))
        .printer_attr("printer-state", Out::Enum(3));
    r.data = b"*PPD-Adobe".to_vec();
    let b = r.encode();
    // A response has the same layout: decode it as one (status = the op id here).
    let d = decode(&b).unwrap();
    assert_eq!(d.status, op::CUPS_ADD_MODIFY_PRINTER);
    let p = d.groups(tag::PRINTER).next().unwrap();
    assert_eq!(p.str("printer-info"), Some("Hall laser"));
    assert_eq!(p.bool("printer-is-accepting-jobs"), Some(true));
    assert_eq!(p.int("printer-state"), Some(3));
    assert!(b.ends_with(b"\x03*PPD-Adobe"));
}

/// Builds a response by hand: groups of `(tag, name, raw value)`.
fn response(status: u16, items: &[(u8, &str, &[u8])]) -> Vec<u8> {
    let mut b = vec![2, 0];
    b.extend_from_slice(&status.to_be_bytes());
    b.extend_from_slice(&[0, 0, 0, 1]);
    for (t, name, raw) in items {
        b.push(*t);
        if *t >= 0x10 {
            b.extend_from_slice(&u16::try_from(name.len()).unwrap().to_be_bytes());
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(&u16::try_from(raw.len()).unwrap().to_be_bytes());
            b.extend_from_slice(raw);
        }
    }
    b.push(tag::END);
    b
}

#[test]
fn decodes_groups_values_and_skips_collections() {
    let lang_text = [&[0u8, 2][..], b"en", &[0, 5], b"Hello"].concat();
    let b = response(
        0x0000,
        &[
            (tag::OPERATION, "", b""),
            (tag::CHARSET, "attributes-charset", b"utf-8"),
            (tag::TEXT, "status-message", b"successful-ok"),
            (tag::PRINTER, "", b""),
            (tag::NAME, "printer-name", b"Hall"),
            (tag::KEYWORD, "printer-state-reasons", b"media-empty-error"),
            (tag::KEYWORD, "", b"toner-low-report"),
            // media-col-default: a collection with a nested one.
            (tag::BEGIN_COLLECTION, "media-col-default", b""),
            (0x4A, "", b"media-size"),
            (tag::BEGIN_COLLECTION, "", b""),
            (0x4A, "", b"x-dimension"),
            (tag::INTEGER, "", &21000i32.to_be_bytes()),
            (tag::END_COLLECTION, "", b""),
            (tag::END_COLLECTION, "", b""),
            (tag::ENUM, "printer-state", &5i32.to_be_bytes()),
            (tag::TEXT_WITH_LANGUAGE, "printer-info", &lang_text),
            (tag::PRINTER, "", b""),
            (tag::NAME, "printer-name", b"PDF"),
            (0x13, "printer-location", b""),
        ],
    );
    let d = decode(&b).unwrap();
    assert!(is_success(d.status));
    assert_eq!(d.message(), Some("successful-ok"));
    let printers: Vec<_> = d.groups(tag::PRINTER).collect();
    assert_eq!(printers.len(), 2);
    assert_eq!(
        printers[0].strs("printer-state-reasons"),
        ["media-empty-error", "toner-low-report"]
    );
    assert_eq!(printers[0].int("printer-state"), Some(5));
    assert_eq!(printers[0].str("printer-info"), Some("Hello"));
    assert_eq!(printers[1].text("printer-name"), "PDF");
    assert_eq!(printers[1].str("printer-location"), None, "no-value");
}

#[test]
fn cut_off_answers_are_errors_not_panics() {
    let b = response(
        0,
        &[
            (tag::PRINTER, "", b""),
            (tag::NAME, "printer-name", b"Hall"),
        ],
    );
    for n in 0..b.len() - 1 {
        assert!(decode(&b[..n]).is_err(), "{n} bytes");
    }
    assert!(decode(&response(0, &[(tag::NAME, "printer-name", b"x")])).is_err());
}
