use sumi_skk::kana::{Converter, Script};
#[test]
fn chunk_boundaries_do_not_change_conversion() {
    for input in [
        "konnichiha",
        "shin'you",
        "gakkou",
        "n",
        "nn",
        "xtsu",
        "東京 de Nihongo\n",
        "kan\r\nky",
    ] {
        let mut whole = Converter::default();
        let expected = format!("{}{}", whole.push(input), whole.finish());
        for boundary in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
            let mut c = Converter::default();
            let actual = format!(
                "{}{}{}",
                c.push(&input[..boundary]),
                c.push(&input[boundary..]),
                c.finish()
            );
            assert_eq!(actual, expected, "{input:?} at {boundary}");
        }
    }
}
#[test]
fn literal_transliteration_has_no_mode_switches_or_composition_markers() {
    let mut c = Converter::default();
    assert_eq!(
        format!("{}{}", c.push("Kanji q l"), c.finish()),
        "かんじ q l"
    );
    assert_eq!(c.pending(), "");
    assert_eq!(c.push("ka"), "か");
    assert_eq!(c.finish(), "");
}
#[test]
fn katakana_and_trailing_pending() {
    let mut c = Converter::new(Script::Katakana);
    assert_eq!(c.push("nihon"), "ニホ");
    assert_eq!(c.finish(), "ン");
    assert_eq!(c.push("ky"), "");
    assert_eq!(c.finish(), "ky");
}
