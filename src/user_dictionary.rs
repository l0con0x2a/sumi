//! Opt-in user dictionaries with advisory locking and atomic replacement.
//! Saving never evaluates dictionary expressions or overwrites unsupported data.
use crate::{Confirmed, dictionary::Dictionary};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug)]
pub struct UserDictionary {
    path: PathBuf,
}
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path
        .file_name()
        .expect("validated dictionary filename")
        .into();
    name.push(suffix);
    path.with_file_name(name)
}
fn read(path: &Path) -> io::Result<(Dictionary, Vec<String>)> {
    let input = match fs::read_to_string(path) {
        Ok(input) => input,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let dictionary =
        Dictionary::parse(&input).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let stats = dictionary.stats();
    if stats.skipped_entries > 0 || stats.skipped_candidates > 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "user dictionary contains unsupported data; use a dedicated supported dictionary",
        ));
    }
    let comments = input
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| {
            line.starts_with(';')
                && !matches!(
                    *line,
                    ";; okuri-ari entries."
                        | ";; okuri-nasi entries."
                        | ";; sumi user dictionary; coding: utf-8"
                )
        })
        .map(str::to_owned)
        .collect();
    Ok((dictionary, comments))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
impl UserDictionary {
    /// Does not create files. Existing symlinks resolve to their target.
    pub fn open(path: impl AsRef<Path>) -> io::Result<(Self, Dictionary)> {
        let path = path.as_ref();
        if path.file_name().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "user dictionary requires a filename",
            ));
        }
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) && !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "dangling user dictionary symlink",
            ));
        }
        let path = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if path.is_absolute() {
                    path.to_owned()
                } else {
                    std::env::current_dir()?.join(path)
                }
            }
            Err(e) => return Err(e),
        };
        let (dictionary, _) = read(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        Ok((Self { path }, dictionary))
    }
    /// Reload while holding a stable sidecar lock, then atomically replace the
    /// file. A busy lock returns WouldBlock rather than stalling key processing.
    pub fn record(&self, confirmed: &Confirmed) -> io::Result<()> {
        crate::dictionary::validate_entry(
            &confirmed.dictionary_key,
            &confirmed.candidate,
            confirmed.okuri.as_deref(),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let parent = self.path.parent().expect("absolute path has a parent");
        fs::create_dir_all(parent)?;
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true);
        options.truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(sibling(&self.path, ".lock"))?;
        lock.try_lock().map_err(io::Error::from)?;
        let (mut dictionary, comments) = read(&self.path)?;
        dictionary
            .promote(
                &confirmed.dictionary_key,
                confirmed.candidate.clone(),
                confirmed.okuri.as_deref(),
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut contents = String::from(";; sumi user dictionary; coding: utf-8\n");
        for comment in comments {
            contents.push_str(&comment);
            contents.push('\n');
        }
        contents.push_str(&dictionary.to_skk());
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let (temporary, mut file) = loop {
            let path = sibling(
                &self.path,
                &format!(
                    ".tmp-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ),
            );
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => break (Temporary(path), file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        };
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary.0, &self.path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        // Keep the sidecar inode: deleting it while another process opens it
        // would allow two independent locks for the same dictionary.
        drop(lock);
        Ok(())
    }
}
