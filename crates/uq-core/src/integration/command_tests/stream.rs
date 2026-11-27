//! NDJSON runs and streamed output.

use super::*;

#[tokio::test]
async fn ndjson_run_collects_every_line() {
    let s = spec("echo '{\"a\":1}'; echo; echo '[2]'", Output::Ndjson, 5000);
    let r = runner().run(&s, &[]).await.unwrap();
    assert_eq!(r.output, serde_json::json!([{"a": 1}, [2]]));
}

#[tokio::test]
async fn ndjson_run_with_a_bad_line_is_an_error() {
    let s = spec("echo '{\"a\":1}'; echo nope", Output::Ndjson, 5000);
    assert!(matches!(
        runner().run(&s, &[]).await,
        Err(CommandError::BadJson(_))
    ));
}

#[tokio::test]
async fn stream_hands_over_each_line_as_it_arrives() {
    let s = spec(
        "echo '{\"n\":1}'; sleep 0.3; printf '{\"n\":2}'",
        Output::Ndjson,
        5000,
    );
    let mut seen: Vec<(Value, Instant)> = Vec::new();
    let start = Instant::now();
    let r = runner()
        .stream(&s, &[], &mut |v| seen.push((v, Instant::now())))
        .await
        .unwrap();
    assert_eq!(r.code, 0);
    assert!(r.output.is_null(), "events are not kept");
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].0["n"], 1);
    assert_eq!(seen[1].0["n"], 2, "a last line with no newline counts");
    assert!(
        seen[0].1.duration_since(start) < Duration::from_millis(250),
        "the first line came before the end"
    );
}

#[tokio::test]
async fn stream_is_not_capped_like_a_read() {
    // 6 MiB of small lines: over the 4 MiB read cap, fine for a stream.
    let s = spec("yes '{\"x\":1}' | head -c 6000000", Output::Ndjson, 20000);
    let mut n = 0usize;
    let r = runner().stream(&s, &[], &mut |_| n += 1).await.unwrap();
    assert_eq!(r.code, 0);
    assert!(n > 400_000);
}

#[tokio::test]
async fn stream_stops_on_a_bad_or_huge_line() {
    let s = spec("echo '{\"a\":1}'; echo nope", Output::Ndjson, 5000);
    assert!(matches!(
        runner().stream(&s, &[], &mut |_| {}).await,
        Err(CommandError::BadJson(_))
    ));
    let s = spec(
        "head -c 70000 /dev/zero | tr '\\0' 'a'; echo",
        Output::Ndjson,
        5000,
    );
    assert_eq!(
        runner().stream(&s, &[], &mut |_| {}).await,
        Err(CommandError::LineTooLarge)
    );
}

#[tokio::test]
async fn stream_needs_an_ndjson_command() {
    let s = spec("true", Output::Json, 1000);
    assert!(matches!(
        runner().stream(&s, &[], &mut |_| {}).await,
        Err(CommandError::BadArgs(_))
    ));
}

#[tokio::test]
async fn stream_reports_a_failing_exit_like_a_read() {
    let s = spec(
        "echo '{\"n\":1}'; echo bad >&2; exit 3",
        Output::Ndjson,
        5000,
    );
    let r = runner().stream(&s, &[], &mut |_| {}).await.unwrap();
    assert_eq!(r.code, 3);
    assert!(r.to_json()["error"].as_str().unwrap().contains("bad"));
}
