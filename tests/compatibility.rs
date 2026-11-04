//! Behavioral cases checked against the upstream references in docs/research.md.
//! These assert observable SKK semantics, independently of editor integration.
use sumi_skk::{Engine, Key, Mode, dictionary::Dictionary};
#[test]
fn mode_commands_and_romaji_rules_have_the_expected_priority() {
    for (input, output, preedit, mode) in [
        ("q", "", "", Mode::Katakana),
        ("lQ", "Q", "", Mode::Ascii),
        ("wwq", "っ", "", Mode::Katakana),
        ("nq", "ん", "", Mode::Katakana),
        ("nl", "ん", "", Mode::Ascii),
        ("zl", "→", "", Mode::Hiragana),
        ("zhzjzk", "←↓↑", "", Mode::Hiragana),
        ("Aiq", "アイ", "", Mode::Hiragana),
        ("qAiq", "あい", "", Mode::Katakana),
        ("Qnq", "ン", "", Mode::Hiragana),
        ("QQ", "", "▽", Mode::Hiragana),
    ] {
        let mut engine = Engine::default();
        let dictionary = Dictionary::demo();
        let actual: String = input
            .chars()
            .map(|c| engine.process_key(Key::Char(c), &dictionary).committed)
            .collect();
        assert_eq!(actual, output, "{input}");
        assert_eq!(engine.snapshot().preedit, preedit, "{input}");
        assert_eq!(engine.snapshot().mode, mode, "{input}");
    }
}
