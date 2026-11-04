use sumi_skk::{Engine, Key, Mode, dictionary::Dictionary, protocol};
fn type_text(e: &mut Engine, text: &str) -> String {
    let d = Dictionary::demo();
    text.chars()
        .map(|c| e.process_key(Key::Char(c), &d).committed)
        .collect()
}
#[test]
fn kana_and_pending() {
    for (input, expected) in [
        ("ka", "か"),
        ("shi", "し"),
        ("konnichiha", "こんにちは"),
        ("gakkou", "がっこう"),
        ("kanpai", "かんぱい"),
        ("shin'you", "しんよう"),
        ("ryokou", "りょこう"),
        ("desu.", "です。"),
        ("ww", "っ"),
    ] {
        let mut e = Engine::default();
        assert_eq!(type_text(&mut e, input), expected, "{input}");
    }
    let mut e = Engine::default();
    assert_eq!(type_text(&mut e, "k"), "");
    assert_eq!(e.snapshot().preedit, "k");
    assert_eq!(type_text(&mut e, "a"), "か");
}
#[test]
fn conversion_and_navigation() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    assert_eq!(type_text(&mut e, "Kanji"), "");
    assert_eq!(e.snapshot().preedit, "▽かんじ");
    let first = e.process_key(Key::Space, &d);
    assert_eq!(first.preedit, "▼漢字");
    assert_eq!(first.candidate_index, Some(0));
    assert_eq!(e.process_key(Key::Space, &d).preedit, "▼感じ");
    assert!(e.process_key(Key::Space, &d).error.is_some());
    assert_eq!(e.process_key(Key::Char('x'), &d).preedit, "▼漢字");
    assert_eq!(e.process_key(Key::Enter, &d).committed, "漢字");
    assert!(e.is_idle());
}
#[test]
fn cancellation_restores_reading_and_backspace_is_utf8_safe() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    type_text(&mut e, "Kanji");
    e.process_key(Key::Space, &d);
    assert_eq!(e.process_key(Key::CtrlG, &d).preedit, "▽かんじ");
    assert_eq!(e.process_key(Key::Backspace, &d).preedit, "▽かん");
    assert_eq!(e.process_key(Key::Escape, &d).preedit, "");
    assert!(e.process_key(Key::Backspace, &d).passthrough.is_some());
    type_text(&mut e, "Kak");
    assert_eq!(e.process_key(Key::Backspace, &d).preedit, "▽か");
}
#[test]
fn mode_switch_and_n_flush() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    type_text(&mut e, "q");
    assert_eq!(e.snapshot().mode, Mode::Katakana);
    assert_eq!(type_text(&mut e, "kan"), "カ");
    assert_eq!(e.process_key(Key::Enter, &d).committed, "ン");
    type_text(&mut e, "l");
    assert_eq!(type_text(&mut e, "ABC"), "ABC");
    e.process_key(Key::CtrlJ, &d);
    assert_eq!(type_text(&mut e, "ka"), "か");
}
#[test]
fn no_candidate_and_unsupported_okuri_do_not_destroy_state() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    type_text(&mut e, "Mumei");
    assert!(e.process_key(Key::Space, &d).error.is_some());
    assert_eq!(e.snapshot().preedit, "▽むめい");
    assert!(e.process_key(Key::Char('K'), &d).error.is_some());
    assert_eq!(e.snapshot().preedit, "▽むめい");
    assert_eq!(e.process_key(Key::Enter, &d).committed, "むめい");
}
#[test]
fn selection_implicitly_commits_then_reprocesses() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    type_text(&mut e, "Kanji");
    e.process_key(Key::Space, &d);
    assert_eq!(e.process_key(Key::Char('k'), &d).committed, "漢字");
    assert_eq!(e.snapshot().preedit, "k");
    assert_eq!(type_text(&mut e, "a"), "か");
}
#[test]
fn dictionary_annotations_duplicates_and_unsupported_entries() {
    let d = Dictionary::parse(
        ";; header\nかんじ /漢字;説明/感じ/漢字/\nおくr /送/[る/送る/]/\nえ /絵/(concat \"a\")/\n",
    )
    .unwrap();
    assert_eq!(d.lookup("かんじ").len(), 2);
    assert_eq!(d.lookup("かんじ")[0].annotation.as_deref(), Some("説明"));
    assert!(d.lookup("おくr").is_empty());
    assert_eq!(d.lookup("え").len(), 1);
    assert!(Dictionary::parse("broken").is_err());
    assert!(Dictionary::parse("かな no-slashes").is_err());
}
#[test]
fn protocol_errors_and_session_isolation() {
    let d = Dictionary::demo();
    let mut a = Engine::default();
    let b = Engine::default();
    assert_eq!(
        protocol::respond(&mut a, &d, r#"{"id":1,"key":"K"}"#)["result"]["preedit"],
        "▽k"
    );
    for bad in [
        "{",
        r#"{"id":2,"key":"F1"}"#,
        r#"{"id":3,"key":"a","oops":1}"#,
        r#"{"key":"a"}"#,
    ] {
        assert!(protocol::respond(&mut a, &d, bad).get("error").is_some());
        assert_eq!(a.snapshot().preedit, "▽k");
    }
    assert_eq!(b.snapshot().preedit, "");
    let r = protocol::respond(&mut a, &d, r#"{"id":"next","key":"a"}"#);
    assert_eq!(r["id"], "next");
    assert_eq!(r["result"]["preedit"], "▽か");
}
#[test]
fn protocol_stream_recovers_after_oversized_and_invalid_utf8() {
    let mut input = vec![b'x'; 70000];
    input.extend_from_slice(b"\n\xff\n{\"id\":1,\"key\":\"a\"}\n");
    let mut output = Vec::new();
    protocol::serve(
        std::io::Cursor::new(input),
        &mut output,
        &Dictionary::demo(),
    )
    .unwrap();
    let rows: Vec<serde_json::Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["error"]["code"], "frame_too_large");
    assert_eq!(rows[1]["error"]["code"], "invalid_utf8");
    assert_eq!(rows[2]["result"]["committed"], "あ");
}
#[test]
fn latency_smoke() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        type_text(&mut e, "Kanji");
        e.process_key(Key::Space, &d);
        e.process_key(Key::Enter, &d);
    }
    eprintln!(
        "70000 keys: {:?}; dictionary/demo helper overhead included",
        start.elapsed()
    );
}
