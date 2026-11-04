use sumi_skk::{Engine, Key, Phase, dictionary::Dictionary};
#[test]
fn mixed_input_never_exposes_an_invalid_candidate_state() {
    let dictionary = Dictionary::demo();
    let mut random = 0x1234_5678u64;
    let keys = [
        Key::Char('a'),
        Key::Char('n'),
        Key::Char('k'),
        Key::Char('q'),
        Key::Char('l'),
        Key::Char('K'),
        Key::Char('N'),
        Key::Char('A'),
        Key::Char('Q'),
        Key::Char('R'),
        Key::Char('u'),
        Key::Space,
        Key::Enter,
        Key::Backspace,
        Key::CtrlG,
        Key::CtrlJ,
        Key::Escape,
        Key::Char('日'),
        Key::Char(' '),
    ];
    for _ in 0..64 {
        let mut engine = Engine::default();
        for c in "Kanji".chars() {
            engine.process_key(Key::Char(c), &dictionary);
        }
        engine.process_key(Key::Space, &dictionary);
        for _ in 0..256 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let update = engine.process_key(
                keys[(random >> 32) as usize % keys.len()].clone(),
                &dictionary,
            );
            match update.phase {
                Phase::Direct => {
                    assert!(update.reading.is_none());
                    assert!(update.okuri.is_none());
                    assert!(update.dictionary_key.is_none());
                }
                Phase::Composing => {
                    assert!(update.reading.is_some());
                    assert!(update.preedit.starts_with('▽'));
                }
                Phase::SelectingCandidates => {
                    assert!(update.pending.is_empty());
                    assert!(update.reading.is_some());
                    assert!(update.candidate_index.unwrap() < update.candidates.len());
                    assert!(update.preedit.starts_with('▼'));
                }
            }
            if update.phase != Phase::SelectingCandidates {
                assert!(update.candidates.is_empty());
                assert!(update.candidate_index.is_none());
            }
            let snapshot = engine.snapshot();
            assert!(snapshot.committed.is_empty());
            assert!(snapshot.confirmed.is_none());
        }
        engine.reset();
        assert!(engine.is_idle());
    }
}
