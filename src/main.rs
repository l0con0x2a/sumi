mod kakoune;
mod terminal;

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};
use sumi_skk::{dictionary::Dictionary, protocol};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = env::args().skip(1).collect();
    let dict_path = if let Some(i) = args.iter().position(|s| s == "--dict") {
        if i + 1 >= args.len() {
            return Err("--dict requires a UTF-8 file".into());
        }
        let path = args.remove(i + 1);
        args.remove(i);
        Some(path)
    } else {
        env::var("SKK_DICTIONARY").ok()
    };
    let dict = match dict_path {
        Some(path) => Dictionary::load(path)?,
        None => Dictionary::demo(),
    };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["convert", reading] => {
            let mut out = io::stdout().lock();
            for candidate in dict.lookup(reading) {
                writeln!(out, "{}", candidate.text)?;
            }
        }
        ["serve", "--stdio"] => protocol::serve(io::stdin().lock(), io::stdout().lock(), &dict)?,
        ["tui"] => {
            if let Some(text) = terminal::run(&dict)? {
                io::stdout().write_all(text.as_bytes())?;
            }
        }
        ["kakoune", directory] => kakoune::run(&dict, directory)?,
        ["--help"] | ["help"] | [] => println!(
            "skk convert READING [--dict FILE]\nskk serve --stdio [--dict FILE]\nskk tui [--dict FILE]\nskk kakoune [--dict FILE]\n\nSKK_DICTIONARY: UTF-8 SKK file; otherwise use the small bundled demo.\nTUI: Enter confirms a conversion; Ctrl-D submits; Esc/Ctrl-C cancels."
        ),
        _ => return Err("invalid arguments; see skk --help".into()),
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("skk: {e}");
            ExitCode::FAILURE
        }
    }
}
