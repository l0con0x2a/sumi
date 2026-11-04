// Original table. Conversion rules are data, independent of UI and dictionaries.
const ROWS: &[(&str, &str)] = &[
    ("", "あいうえお"),
    ("k", "かきくけこ"),
    ("g", "がぎぐげご"),
    ("s", "さしすせそ"),
    ("z", "ざじずぜぞ"),
    ("t", "たちつてと"),
    ("d", "だぢづでど"),
    ("n", "なにぬねの"),
    ("h", "はひふへほ"),
    ("b", "ばびぶべぼ"),
    ("p", "ぱぴぷぺぽ"),
    ("m", "まみむめも"),
    ("r", "らりるれろ"),
    ("x", "ぁぃぅぇぉ"),
    ("l", "ぁぃぅぇぉ"),
];
const EXTRA: &[(&str, &str)] = &[
    ("shi", "し"),
    ("chi", "ち"),
    ("tsu", "つ"),
    ("fu", "ふ"),
    ("ji", "じ"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("kya", "きゃ"),
    ("kyu", "きゅ"),
    ("kyo", "きょ"),
    ("gya", "ぎゃ"),
    ("gyu", "ぎゅ"),
    ("gyo", "ぎょ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("sho", "しょ"),
    ("she", "しぇ"),
    ("sya", "しゃ"),
    ("syu", "しゅ"),
    ("syo", "しょ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("jo", "じょ"),
    ("je", "じぇ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("zya", "じゃ"),
    ("zyu", "じゅ"),
    ("zyo", "じょ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("che", "ちぇ"),
    ("tya", "ちゃ"),
    ("tyu", "ちゅ"),
    ("tyo", "ちょ"),
    ("dya", "ぢゃ"),
    ("dyu", "ぢゅ"),
    ("dyo", "ぢょ"),
    ("nya", "にゃ"),
    ("nyu", "にゅ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyu", "ひゅ"),
    ("hyo", "ひょ"),
    ("bya", "びゃ"),
    ("byu", "びゅ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyu", "ぴゅ"),
    ("pyo", "ぴょ"),
    ("mya", "みゃ"),
    ("myu", "みゅ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryu", "りゅ"),
    ("ryo", "りょ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("xa", "ぁ"),
    ("xtu", "っ"),
    ("xtsu", "っ"),
    ("ltu", "っ"),
    ("ltsu", "っ"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("xwa", "ゎ"),
    ("-", "ー"),
    (",", "、"),
    (".", "。"),
];
fn table() -> &'static Vec<(String, String)> {
    static TABLE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut entries: Vec<_> = EXTRA
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        for (prefix, kana) in ROWS {
            for (vowel, kana) in "aiueo".chars().zip(kana.chars()) {
                entries.push((format!("{prefix}{vowel}"), kana.to_string()));
            }
        }
        entries
    })
}

pub fn feed(pending: &mut String, c: char) -> String {
    let mut output = String::new();
    // Defer nn until the next key: nni -> んに, nn<Enter> -> ん.
    if pending == "nn" {
        output.push('ん');
        pending.clear();
        if "aiueoy".contains(c) {
            pending.push('n');
        }
    }
    if pending == "n" && c != 'n' && c != '\'' && !"aiueoy".contains(c) {
        output.push('ん');
        pending.clear();
    }
    if pending == "n" && c == '\'' {
        pending.clear();
        output.push('ん');
        return output;
    }
    if pending.len() == 1
        && pending.starts_with(c)
        && c.is_ascii_lowercase()
        && !"aeioun".contains(c)
    {
        output.push('っ');
        return output;
    }
    pending.push(c);
    if pending == "n" || pending == "nn" {
        return output;
    }
    if let Some((_, kana)) = table().iter().find(|(key, _)| key == pending) {
        output.push_str(kana);
        pending.clear();
    } else if !table()
        .iter()
        .any(|(key, _)| key.starts_with(pending.as_str()))
    {
        // Invalid sequences preserve their literal bytes rather than losing input.
        let last = pending.pop().unwrap();
        output.push_str(pending);
        pending.clear();
        if table().iter().any(|(key, _)| key.starts_with(last)) || last == 'n' {
            pending.push(last);
        } else {
            output.push(last);
        }
    }
    output
}

pub fn flush(pending: &mut String) -> String {
    if pending == "n" || pending == "nn" {
        pending.clear();
        "ん".into()
    } else {
        std::mem::take(pending)
    }
}
pub fn katakana(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('ぁ'..='ゖ').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap()
            } else {
                c
            }
        })
        .collect()
}
