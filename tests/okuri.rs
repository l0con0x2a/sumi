use sumi_skk::{Engine, Key, Mode, Phase, dictionary::Dictionary};
fn input(engine: &mut Engine, dictionary: &Dictionary, keys: &str) -> String {
    keys.chars()
        .map(|c| engine.process_key(Key::Char(c), dictionary).committed)
        .collect()
}
#[test]
fn okuri_automatically_selects_and_commits_the_suffix() {
    let d = Dictionary::demo();
    for (keys, key, preedit, committed) in [
        ("OkuRu", "おくr", "▼送る", "送る"),
        ("KaKu", "かk", "▼書く", "書く"),
        ("KangaE", "かんがe", "▼考え", "考え"),
        ("KanJi", "かんj", "▼感じ", "感じ"),
        ("TukaTte", "つかt", "▼使って", "使って"),
    ] {
        let mut e = Engine::default();
        assert_eq!(input(&mut e, &d, keys), "", "{keys}");
        let s = e.snapshot();
        assert_eq!(s.dictionary_key.as_deref(), Some(key));
        assert_eq!(s.preedit, preedit);
        assert_eq!(s.phase, Phase::SelectingCandidates);
        assert_eq!(e.process_key(Key::Enter, &d).committed, committed);
        assert!(e.is_idle());
    }
}
#[test]
fn hints_navigation_and_cancel_preserve_okuri() {
    let d = Dictionary::parse("おくr /送/贈/[る/贈/]/").unwrap();
    let mut e = Engine::default();
    input(&mut e, &d, "OkuRu");
    assert_eq!(e.snapshot().preedit, "▼贈る");
    assert_eq!(e.process_key(Key::Space, &d).preedit, "▼送る");
    assert_eq!(e.process_key(Key::CtrlG, &d).preedit, "▽おく*る");
    assert_eq!(e.process_key(Key::Space, &d).preedit, "▼贈る");
    e.process_key(Key::CtrlG, &d);
    assert_eq!(e.process_key(Key::Backspace, &d).preedit, "▽おく");
    input(&mut e, &d, "R");
    assert_eq!(e.snapshot().preedit, "▽おく*r");
    assert_eq!(e.process_key(Key::Backspace, &d).preedit, "▽おく");
    assert_eq!(e.process_key(Key::CtrlG, &d).preedit, "");
}
#[test]
fn uppercase_completing_pending_kana_does_not_start_okuri() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    input(&mut e, &d, "NA");
    assert_eq!(e.snapshot().preedit, "▽な");
    assert_eq!(e.snapshot().okuri, None);
    e.reset();
    input(&mut e, &d, "QKa");
    assert_eq!(e.snapshot().preedit, "▽か");
    e.reset();
    input(&mut e, &d, "Q");
    assert_eq!(e.snapshot().preedit, "▽");
}
#[test]
fn unknown_okuri_can_be_edited_or_committed_as_kana() {
    let mut e = Engine::default();
    let d = Dictionary::default();
    input(&mut e, &d, "OkuR");
    let u = e.process_key(Key::Char('u'), &d);
    assert_eq!(u.preedit, "▽おく*る");
    assert!(u.error.is_some());
    assert_eq!(e.process_key(Key::Enter, &d).committed, "おくる");
    e = Engine::new(Mode::Katakana);
    input(&mut e, &d, "OkuRu");
    assert_eq!(e.process_key(Key::CtrlJ, &d).committed, "オクル");
    assert_eq!(e.snapshot().mode, Mode::Hiragana);
}
#[test]
fn n_and_sokuon_are_preserved_across_okuri_boundary() {
    let d = Dictionary::parse("ふn /踏/\nさs /察/\n").unwrap();
    let mut e = Engine::default();
    input(&mut e, &d, "FuNda");
    assert_eq!(e.snapshot().preedit, "▼踏んだ");
    assert_eq!(e.process_key(Key::Enter, &d).committed, "踏んだ");
    input(&mut e, &d, "SaSsu");
    assert_eq!(e.snapshot().preedit, "▼察っす");
}
#[test]
fn mode_switch_flushes_pending_n_and_ctrl_j_confirms() {
    let d = Dictionary::demo();
    let mut e = Engine::default();
    assert_eq!(input(&mut e, &d, "nq"), "ん");
    assert_eq!(e.snapshot().mode, Mode::Katakana);
    e = Engine::default();
    input(&mut e, &d, "Kanji");
    e.process_key(Key::Space, &d);
    assert_eq!(e.process_key(Key::CtrlJ, &d).committed, "漢字");
    assert!(e.is_idle());
    assert!(e.process_key(Key::Char('\n'), &d).error.is_some());
    assert!(e.is_idle());
}
