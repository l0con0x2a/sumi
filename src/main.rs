#[cfg(all(unix, feature = "terminal"))]
mod terminal;

use std::{
    env,
    ffi::{OsStr, OsString},
    io::{self, BufRead, Write},
    path::PathBuf,
    process::ExitCode,
};
use sumi_skk::{
    Confirmed, Engine, Mode, RomajiRule,
    dictionary::{Candidate, Dictionary},
    kana::{Converter, Script},
    protocol,
    session::Session,
    user_dictionary::UserDictionary,
};

const HELP: &str = "skk — standalone SKK input and kana conversion

Usage:
  skk kana [--katakana] [TEXT]
  skk convert [--json] [--dict FILE]... READING
  skk check-dict FILE
  skk register --user-dict FILE [--okuri SUFFIX] READING TEXT
  skk serve --stdio [--dict FILE]...
  skk tui [--dict FILE]...
  skk --help
  skk --version

--romaji skk|common selects double-n behavior (kana: common; serve/tui: skk).
--user-dict FILE enables candidate learning for serve/tui and priority lookup for convert.

kana:    Transliterate literal romaji; read UTF-8 stdin when TEXT is omitted.
convert: Print dictionary candidates, one per line; --json includes annotations.
check-dict: Validate a UTF-8 dictionary and print JSON counts for supported data.
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
    words: Vec<OsString>,
    dictionaries: Vec<PathBuf>,
    json: bool,
    katakana: bool,
    romaji: Option<RomajiRule>,
    user_dictionary: Option<PathBuf>,
    okuri: Option<String>,
}
fn utf8<'a>(word: &'a OsStr, name: &str) -> io::Result<&'a str> {
    word.to_str()
        .ok_or_else(|| usage(format!("{name} must be UTF-8")))
}
fn options(args: impl Iterator<Item = OsString>) -> io::Result<Options> {
    let mut options = Options::default();
    let mut args = args.peekable();
    let mut positional = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") if !positional => positional = true,
            Some("--dict") if !positional => {
                let path = args
                    .next()
                    .ok_or_else(|| usage("--dict requires a UTF-8 dictionary file"))?;
                options.dictionaries.push(path.into());
            }
            Some("--json") if !positional => options.json = true,
            Some("--user-dict") if !positional => {
                options.user_dictionary = Some(
                    args.next()
                        .ok_or_else(|| usage("--user-dict requires a filename"))?
                        .into(),
                )
            }
            Some("--okuri") if !positional => {
                options.okuri = Some(
                    utf8(
                        &args
                            .next()
                            .ok_or_else(|| usage("--okuri requires a suffix"))?,
                        "--okuri",
                    )?
                    .to_owned(),
                )
            }
            Some("--katakana") if !positional => options.katakana = true,
            Some("--romaji") if !positional => {
                options.romaji = Some(match args.next().as_ref().and_then(|s| s.to_str()) {
                    Some("skk") => RomajiRule::Skk,
                    Some("common") => RomajiRule::Common,
                    _ => return Err(usage("--romaji requires skk or common")),
                });
            }
            Some("--stdio") if !positional => options.words.push(arg),
            _ if !positional && arg.as_encoded_bytes().starts_with(b"-") => {
                return Err(usage(format!("unknown option: {}", arg.to_string_lossy())));
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
fn session(options: &Options) -> io::Result<Session> {
    let engine = Engine::with_romaji_rule(Mode::Hiragana, options.romaji.unwrap_or_default());
    let dictionary = dictionaries(&options.dictionaries)?;
    match &options.user_dictionary {
        Some(path) => Session::with_user_dictionary(engine, dictionary, path),
        None => Ok(Session::new(engine, dictionary)),
    }
}
fn kana(text: Option<&str>, script: Script, rule: RomajiRule) -> io::Result<()> {
    let mut converter = Converter::with_rule(script, rule);
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
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty()
        || args
            .iter()
            .take_while(|s| s.as_os_str() != "--")
            .any(|s| s == "--help" || s == "-h")
        || args == ["help"]
    {
        return io::stdout().lock().write_all(HELP.as_bytes());
    }
    if args == ["--version"] {
        return writeln!(io::stdout().lock(), "skk {}", env!("CARGO_PKG_VERSION"));
    }
    let command = utf8(&args[0], "command")?;
    let options = options(args[1..].iter().cloned())?;
    match command {
        "kana" => {
            if options.json
                || !options.dictionaries.is_empty()
                || options.user_dictionary.is_some()
                || options.okuri.is_some()
                || options.words.len() > 1
            {
                return Err(usage("usage: skk kana [--katakana] [TEXT]"));
            }
            kana(
                options
                    .words
                    .first()
                    .map(|word| utf8(word, "TEXT"))
                    .transpose()?,
                if options.katakana {
                    Script::Katakana
                } else {
                    Script::Hiragana
                },
                options.romaji.unwrap_or(RomajiRule::Common),
            )
        }
        "convert"
            if !options.katakana
                && options.romaji.is_none()
                && options.okuri.is_none()
                && options.words.len() == 1 =>
        {
            let reading = utf8(&options.words[0], "READING")?;
            let session = session(&options)?;
            let candidates = session.dictionary().lookup(reading);
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
        "check-dict"
            if !options.katakana
                && !options.json
                && options.romaji.is_none()
                && options.user_dictionary.is_none()
                && options.okuri.is_none()
                && options.dictionaries.is_empty()
                && options.words.len() == 1 =>
        {
            let dictionary = Dictionary::load(&options.words[0])?;
            let mut out = io::stdout().lock();
            serde_json::to_writer(&mut out, &dictionary.stats())?;
            writeln!(out)
        }
        "register"
            if options.user_dictionary.is_some()
                && !options.json
                && !options.katakana
                && options.romaji.is_none()
                && options.dictionaries.is_empty()
                && options.words.len() == 2 =>
        {
            let confirmed = Confirmed {
                dictionary_key: utf8(&options.words[0], "READING")?.into(),
                candidate: Candidate {
                    text: utf8(&options.words[1], "TEXT")?.into(),
                    annotation: None,
                },
                okuri: options.okuri,
            };
            let (user, _) = UserDictionary::open(options.user_dictionary.as_ref().unwrap())?;
            user.record(&confirmed)
        }
        "serve"
            if !options.json
                && !options.katakana
                && options.okuri.is_none()
                && options.words == ["--stdio"] =>
        {
            protocol::serve_session(io::stdin().lock(), io::stdout().lock(), session(&options)?)
        }
        "tui"
            if !options.json
                && !options.katakana
                && options.okuri.is_none()
                && options.words.is_empty() =>
        {
            #[cfg(all(unix, feature = "terminal"))]
            {
                if let Some(text) = terminal::run(session(&options)?)? {
                    io::stdout().lock().write_all(text.as_bytes())?;
                }
                Ok(())
            }
            #[cfg(not(all(unix, feature = "terminal")))]
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "tui requires Unix and the terminal feature",
            ))
        }
        "convert" | "serve" | "tui" | "check-dict" | "register" => Err(usage(format!(
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
            #[cfg(all(unix, feature = "terminal"))]
            if let Some(signal) = terminal::interrupted_signal() {
                return ExitCode::from(128 + signal as u8);
            }
            ExitCode::from(if error.kind() == io::ErrorKind::InvalidInput {
                2
            } else {
                1
            })
        }
    }
}
