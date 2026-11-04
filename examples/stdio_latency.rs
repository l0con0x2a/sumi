//! Measure the public CLI protocol, including IPC and JSON parsing, with one
//! persistent child process. Does not use the engine's Rust API.
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::Instant,
};
struct Client {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}
impl Client {
    fn request(&mut self, mut request: Value) -> io::Result<Value> {
        self.id += 1;
        request["id"] = json!(self.id);
        writeln!(self.input, "{request}")?;
        self.input.flush()?;
        let mut line = String::new();
        if self.output.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "skk closed stdout",
            ));
        }
        let reply: Value = serde_json::from_str(&line)?;
        if reply["id"] != self.id || reply.get("result").is_none() {
            return Err(io::Error::other(format!("bad response: {reply}")));
        }
        Ok(reply)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let executable = args.next().unwrap_or_else(|| "skk".into());
    let mut command = Command::new(executable);
    command.args(["serve", "--stdio"]);
    if let Some(dictionary) = args.next() {
        command.arg("--dict").arg(dictionary);
    }
    if args.next().is_some() {
        return Err("usage: stdio_latency [SKK_BINARY [DICTIONARY]]".into());
    }
    let start = Instant::now();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut client = Client {
        input: child.stdin.take().unwrap(),
        output: BufReader::new(child.stdout.take().unwrap()),
        child,
        id: 0,
    };
    client.request(json!({"op":"state"}))?;
    eprintln!("process startup and dictionary load: {:?}", start.elapsed());
    let mut samples = Vec::with_capacity(70000);
    for _ in 0..10000 {
        for key in ["K", "a", "n", "j", "i", "Space", "Enter"] {
            let start = Instant::now();
            std::hint::black_box(client.request(json!({"key":key}))?);
            samples.push(start.elapsed().as_nanos());
        }
    }
    samples.sort_unstable();
    println!(
        "{} requests: p50={}ns p95={}ns p99={}ns max={}ns",
        samples.len(),
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() * 99 / 100],
        samples.last().unwrap()
    );
    Ok(())
}
