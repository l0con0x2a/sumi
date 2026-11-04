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
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
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

#[test]
fn option_terminator_preserves_help_as_a_literal_reading() {
    let output = skk().args(["convert", "--", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn registration_and_session_learning_work_through_public_commands() {
    let directory = std::env::temp_dir().join(format!("sumi-cli-learning-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("user.skk");
    let registered = skk()
        .args(["register", "--user-dict"])
        .arg(&path)
        .args(["あたらしい", "新しい語"])
        .output()
        .unwrap();
    assert!(
        registered.status.success(),
        "{}",
        String::from_utf8_lossy(&registered.stderr)
    );
    assert!(registered.stdout.is_empty());
    let search = skk()
        .args(["convert", "--user-dict"])
        .arg(&path)
        .arg("あたらしい")
        .output()
        .unwrap();
    assert_eq!(search.stdout, "新しい語\n".as_bytes());
    let mut child = skk()
        .args(["serve", "--stdio", "--user-dict"])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for (id, key) in ["K", "a", "n", "j", "i", "Space", "Space", "Enter"]
        .iter()
        .enumerate()
    {
        writeln!(input, "{}", serde_json::json!({"id":id,"key":key})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let rows: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.last().unwrap()["result"]["committed"], "感じ");
    assert_eq!(
        rows.last().unwrap()["result"]["learning_error"],
        serde_json::Value::Null
    );
    let search = skk()
        .args(["convert", "--user-dict"])
        .arg(&path)
        .arg("かんじ")
        .output()
        .unwrap();
    assert_eq!(search.stdout, "感じ\n漢字\n".as_bytes());
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_supported_but_text_arguments_are_validated() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let directory = std::env::temp_dir().join(format!("sumi-cli-bytes-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let dictionary = directory.join(OsString::from_vec(b"dict-\xff.skk".to_vec()));
    let user = directory.join(OsString::from_vec(b"user-\xfe.skk".to_vec()));
    std::fs::write(&dictionary, "かんじ /漢字/\n").unwrap();
    let output = skk()
        .args(["convert", "--dict"])
        .arg(&dictionary)
        .arg("かんじ")
        .output()
        .unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, "漢字\n".as_bytes());
    assert!(
        skk()
            .arg("check-dict")
            .arg(&dictionary)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        skk()
            .args(["register", "--user-dict"])
            .arg(&user)
            .args(["かな", "仮名"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let invalid = skk()
        .arg("kana")
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("UTF-8"));
    std::fs::remove_dir_all(directory).unwrap();
}
