use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
fn skk() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_skk"));
    c.env_remove("SKK_DICTIONARY");
    c
}
#[test]
fn convert_is_plain_text_and_missing_dictionary_is_an_error() {
    let out = skk().args(["convert", "かんじ"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "漢字\n感じ\n");
    assert!(
        skk()
            .args(["convert", "かんじ", "--dict", "/does-not-exist/sumi.skk"])
            .output()
            .unwrap()
            .status
            .code()
            .is_some_and(|c| c != 0)
    );
}
#[test]
fn stdio_sessions_flush_results_and_are_independent() {
    for _ in 0..2 {
        let mut child = skk()
            .args(["serve", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut rows = Vec::<serde_json::Value>::new();
        for (id, key) in ["K", "a"].iter().enumerate() {
            writeln!(input, "{{\"id\":{id},\"key\":\"{key}\"}}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            rows.push(serde_json::from_str(&line).unwrap());
        }
        drop(input);
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        assert_eq!(rows[0]["result"]["preedit"], "▽k");
        assert_eq!(rows[1]["result"]["preedit"], "▽か");
        assert!(out.stderr.is_empty());
    }
}

#[test]
fn help_version_and_kana_do_not_load_a_dictionary() {
    for args in [vec!["--help"], vec!["--version"], vec!["kana", "Kanji"]] {
        let out = skk()
            .env("SKK_DICTIONARY", "/does-not-exist/sumi.skk")
            .args(&args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
    }
    let out = skk().args(["kana", "konnichiha"]).output().unwrap();
    assert_eq!(out.stdout, "こんにちは\n".as_bytes());
    let out = skk()
        .args(["kana", "--katakana", "nihon"])
        .output()
        .unwrap();
    assert_eq!(out.stdout, "ニホン\n".as_bytes());
    assert_eq!(
        skk()
            .args(["kana", "--json", "ka"])
            .status()
            .unwrap()
            .code(),
        Some(2)
    );
}
#[test]
fn kana_stdin_preserves_line_endings_and_flushes_eof() {
    let mut child = skk()
        .args(["kana"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("kan\r\n日本 konnichiha\nn".as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, "かん\r\n日本 こんにちは\nん".as_bytes());
}
#[test]
fn json_search_and_dictionary_priority() {
    let directory = std::env::temp_dir().join(format!("sumi-cli-dicts-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let first = directory.join("first.skk");
    let second = directory.join("second.skk");
    std::fs::write(&first, "かんじ /感じ;優先/漢字/\n").unwrap();
    std::fs::write(&second, "かんじ /漢字;重複/幹事/\n").unwrap();
    let output = skk()
        .args(["convert", "--json", "--dict"])
        .arg(&first)
        .arg("--dict")
        .arg(&second)
        .arg("かんじ")
        .output()
        .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(output.status.success());
    let candidates: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(candidates.as_array().unwrap().len(), 3);
    assert_eq!(candidates[0]["text"], "感じ");
    assert_eq!(candidates[0]["annotation"], "優先");
    assert_eq!(candidates[1]["text"], "漢字");
    assert_eq!(candidates[1]["annotation"], serde_json::Value::Null);
    assert_eq!(candidates[2]["text"], "幹事");
}

#[test]
fn kana_supports_explicit_skk_and_common_rules() {
    let skk_result = skk()
        .args(["kana", "--romaji", "skk", "nni"])
        .output()
        .unwrap();
    let common_result = skk()
        .args(["kana", "--romaji", "common", "nni"])
        .output()
        .unwrap();
    assert_eq!(skk_result.stdout, "んい\n".as_bytes());
    assert_eq!(common_result.stdout, "んに\n".as_bytes());
    assert_eq!(
        skk()
            .args(["kana", "--romaji", "invalid"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}
