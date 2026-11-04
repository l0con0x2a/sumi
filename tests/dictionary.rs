use sumi_skk::dictionary::Dictionary;
#[test]
fn escaped_candidates_are_data_not_lisp() {
    let d = Dictionary::parse(r#"きごう /(concat "a\057b\073c");記号/(concat "\343\201\202")/(shell-command "touch /tmp/sumi-test-should-not-exist")/普通/
"#).unwrap();
    let candidates = d.lookup("きごう");
    assert_eq!(candidates.len(), 3);
    assert_eq!(candidates[0].text, "a/b;c");
    assert_eq!(candidates[0].annotation.as_deref(), Some("記号"));
    assert_eq!(candidates[1].text, "あ");
    assert_eq!(candidates[2].text, "普通");
    assert_eq!(d.stats().skipped_candidates, 1);
}
#[test]
fn okuri_blocks_do_not_leak_and_are_prioritized() {
    let d = Dictionary::parse("おくr /送/贈;一般/[る/贈;ヒント/]/[れ/遅/]/\n").unwrap();
    assert_eq!(d.lookup("おくr").len(), 2);
    let candidates = d.lookup_okuri("おくr", "る");
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].text, "贈");
    assert_eq!(candidates[0].annotation.as_deref(), Some("ヒント"));
    assert_eq!(candidates[1].text, "送");
    assert_eq!(d.lookup_okuri("おくr", "ろ")[0].text, "送");
    assert_eq!(d.stats().okuri_blocks, 2);
}
#[test]
fn malformed_input_returns_errors_instead_of_panicking() {
    for values in [
        "/",
        "missing",
        "/[る/送/",
        "/送/]/",
        "/[る/[れ/送/]/]/",
        "/(concat \"abc)/",
    ] {
        assert!(
            Dictionary::parse(&format!("かな {values}")).is_err(),
            "{values}"
        );
    }
    for n in 0..512 {
        let input = format!("かな /{}", "(".repeat(n));
        assert!(std::panic::catch_unwind(|| Dictionary::parse(&input)).is_ok());
    }
}
#[test]
fn bom_crlf_and_merge_preserve_priority() {
    let mut a = Dictionary::parse("\u{feff};; test\r\nおくr /送/[る/贈;先/]/\r\n").unwrap();
    a.extend(Dictionary::parse("おくr /遅/送;後/[る/贈;後/送/]/\nabc /英/\n").unwrap());
    let candidates = a.lookup_okuri("おくr", "る");
    assert_eq!(
        candidates
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["贈", "送", "遅"]
    );
    assert_eq!(candidates[0].annotation.as_deref(), Some("先"));
    assert_eq!(a.stats().skipped_entries, 1);
}

#[test]
fn annotation_parentheses_are_literal_text() {
    let d = Dictionary::parse("はr /貼;(paste) 糊/膨;≒腫れる (NB:未閉じ/\n").unwrap();
    assert_eq!(d.lookup("はr")[0].annotation.as_deref(), Some("(paste) 糊"));
    assert_eq!(
        d.lookup("はr")[1].annotation.as_deref(),
        Some("≒腫れる (NB:未閉じ")
    );
}
