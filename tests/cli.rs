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
