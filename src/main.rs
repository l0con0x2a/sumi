mod terminal;

use std::{
    env,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::ExitCode,
};
use sumi_skk::{
    dictionary::Dictionary,
    kana::{Converter, Script},
    protocol,
};

const HELP: &str = "skk — standalone SKK input and kana conversion

Usage:
  skk kana [--katakana] [TEXT]
  skk convert [--json] [--dict FILE]... READING
  skk serve --stdio [--dict FILE]...
  skk tui [--dict FILE]...
  skk --help
  skk --version

kana:    Transliterate literal romaji; read UTF-8 stdin when TEXT is omitted.
convert: Print dictionary candidates, one per line; --json includes annotations.
serve:   One stateful JSON Lines session on stdin/stdout.
tui:     Compose text in a terminal; Ctrl-D sends, Esc/Ctrl-C cancels.

--dict may be repeated; earlier dictionaries take priority.
SKK_DICTIONARY selects a UTF-8 SKK file when --dict is absent.
Without a dictionary, convert/serve/tui use the small bundled demo.
kana, help and version do not load a dictionary.
";
fn usage(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
#[derive(Default)]
struct Options {
    words: Vec<String>,
    dictionaries: Vec<PathBuf>,
    json: bool,
    katakana: bool,
}
fn options(args: impl Iterator<Item = String>) -> io::Result<Options> {
    let mut options = Options::default();
    let mut args = args.peekable();
    let mut positional = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" if !positional => positional = true,
            "--dict" if !positional => {
                let path = args
                    .next()
                    .ok_or_else(|| usage("--dict requires a UTF-8 dictionary file"))?;
                options.dictionaries.push(path.into());
            }
            "--json" if !positional => options.json = true,
            "--katakana" if !positional => options.katakana = true,
            "--stdio" if !positional => options.words.push(arg),
            _ if !positional && arg.starts_with('-') => {
                return Err(usage(format!("unknown option: {arg}")));
            }
            _ => options.words.push(arg),
        }
    }
    Ok(options)
}
fn dictionaries(paths: &[PathBuf]) -> io::Result<Dictionary> {
    let paths = if paths.is_empty() {
        env::var_os("SKK_DICTIONARY")
            .map(PathBuf::from)
            .into_iter()
            .collect()
    } else {
        paths.to_vec()
    };
    if paths.is_empty() {
        return Ok(Dictionary::demo());
    }
    let mut dictionary = Dictionary::default();
    for path in paths {
        dictionary.extend(Dictionary::load(&path)?);
    }
    Ok(dictionary)
}
fn kana(text: Option<&String>, script: Script) -> io::Result<()> {
    let mut converter = Converter::new(script);
    let mut out = io::stdout().lock();
    if let Some(text) = text {
        write!(out, "{}{}", converter.push(text), converter.finish())?;
        writeln!(out)?;
    } else {
        let mut input = io::stdin().lock();
        let mut line = String::new();
        while input.read_line(&mut line)? != 0 {
            write!(out, "{}", converter.push(&line))?;
            out.flush()?;
            line.clear();
        }
        write!(out, "{}", converter.finish())?;
    }
    out.flush()
}
fn run() -> io::Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|s| s == "--help" || s == "-h") || args == ["help"] {
        return io::stdout().lock().write_all(HELP.as_bytes());
    }
    if args == ["--version"] {
        return writeln!(io::stdout().lock(), "skk {}", env!("CARGO_PKG_VERSION"));
    }
    let command = &args[0];
    let options = options(args[1..].iter().cloned())?;
    match command.as_str() {
        "kana" => {
            if options.json || !options.dictionaries.is_empty() || options.words.len() > 1 {
                return Err(usage("usage: skk kana [--katakana] [TEXT]"));
            }
            kana(
                options.words.first(),
                if options.katakana {
                    Script::Katakana
                } else {
                    Script::Hiragana
                },
            )
        }
        "convert" if !options.katakana && options.words.len() == 1 => {
            let dictionary = dictionaries(&options.dictionaries)?;
            let candidates = dictionary.lookup(&options.words[0]);
            let mut out = io::stdout().lock();
            if options.json {
                serde_json::to_writer(&mut out, candidates)?;
                writeln!(out)
            } else {
                for candidate in candidates {
                    writeln!(out, "{}", candidate.text)?;
                }
                Ok(())
            }
        }
        "serve" if !options.json && !options.katakana && options.words == ["--stdio"] => {
            protocol::serve(
                io::stdin().lock(),
                io::stdout().lock(),
                &dictionaries(&options.dictionaries)?,
            )
        }
        "tui" if !options.json && !options.katakana && options.words.is_empty() => {
            if let Some(text) = terminal::run(&dictionaries(&options.dictionaries)?)? {
                io::stdout().lock().write_all(text.as_bytes())?;
            }
            Ok(())
        }
        "convert" | "serve" | "tui" => Err(usage(format!(
            "invalid arguments for {command}; see skk --help"
        ))),
        _ => Err(usage(format!("unknown command: {command}; see skk --help"))),
    }
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("skk: {error}");
            ExitCode::from(if error.kind() == io::ErrorKind::InvalidInput {
                2
            } else {
                1
            })
        }
    }
}
