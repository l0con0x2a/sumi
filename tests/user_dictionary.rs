use std::{
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use sumi_skk::{
    Confirmed, Engine, Key,
    dictionary::{Candidate, Dictionary},
    session::Session,
    user_dictionary::UserDictionary,
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "sumi-user-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> PathBuf {
        self.0.join("user.skk")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn choice(reading: &str, text: &str, okuri: Option<&str>) -> Confirmed {
    Confirmed {
        dictionary_key: reading.into(),
        candidate: Candidate {
            text: text.into(),
            annotation: Some("説明;注釈/記号".into()),
        },
        okuri: okuri.map(str::to_owned),
    }
}
fn type_text(s: &mut Session, text: &str) {
    for c in text.chars() {
        s.process_key(Key::Char(c));
    }
}
#[test]
fn serialization_roundtrips_reserved_characters_and_okuri() {
    let mut d = Dictionary::default();
    for (key, word, suffix) in [
        ("きごう", "a/b;c\\\"", None),
        ("おくr", "贈", Some("る")),
        ("えもじ", "😀", None),
        ("きごう", "[括弧]", None),
    ] {
        let c = choice(key, word, suffix);
        d.promote(key, c.candidate, suffix).unwrap();
    }
    let output = d.to_skk();
    let decoded = Dictionary::parse(&output).unwrap();
    for key in ["きごう", "おくr", "えもじ"] {
        assert_eq!(decoded.lookup(key), d.lookup(key));
    }
    assert_eq!(
        decoded.lookup_okuri("おくr", "る"),
        d.lookup_okuri("おくr", "る")
    );
    assert_eq!(decoded.to_skk(), output);
}
#[test]
fn open_is_read_only_and_successful_learning_survives_a_new_session() {
    let directory = Directory::new();
    let path = directory.0.join("nested/user.skk");
    let mut session =
        Session::with_user_dictionary(Engine::default(), Dictionary::demo(), &path).unwrap();
    assert!(!path.parent().unwrap().exists());
    type_text(&mut session, "Kanji");
    session.process_key(Key::Space);
    session.process_key(Key::Space);
    let update = session.commit();
    assert_eq!(update.committed, "感じ");
    assert!(update.learning_error.is_none());
    assert!(path.exists());
    let mut next =
        Session::with_user_dictionary(Engine::default(), Dictionary::demo(), &path).unwrap();
    type_text(&mut next, "Kanji");
    assert_eq!(next.process_key(Key::Space).preedit, "▼感じ");
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 2); // dictionary and stable lock
}
#[test]
fn stale_store_instances_merge_under_the_lock_and_keep_comments() {
    let directory = Directory::new();
    let path = directory.path();
    fs::write(
        &path,
        ";; User note and copyright\n;; okuri-nasi entries.\nあい /愛/\n",
    )
    .unwrap();
    let (first, _) = UserDictionary::open(&path).unwrap();
    let (second, _) = UserDictionary::open(&path).unwrap();
    first.record(&choice("おくr", "贈", Some("る"))).unwrap();
    second.record(&choice("かんじ", "感じ", None)).unwrap();
    let d = Dictionary::load(&path).unwrap();
    assert_eq!(d.lookup("あい")[0].text, "愛");
    assert_eq!(d.lookup_okuri("おくr", "る")[0].text, "贈");
    assert_eq!(d.lookup("かんじ")[0].text, "感じ");
    assert_eq!(
        fs::read_to_string(path)
            .unwrap()
            .matches("User note and copyright")
            .count(),
        1
    );
}
#[test]
fn lock_contention_reports_failure_without_losing_the_commit() {
    let directory = Directory::new();
    let path = directory.path();
    let mut session =
        Session::with_user_dictionary(Engine::default(), Dictionary::demo(), &path).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.0.join("user.skk.lock"))
        .unwrap();
    lock.lock().unwrap();
    type_text(&mut session, "Kanji");
    session.process_key(Key::Space);
    session.process_key(Key::Space);
    let update = session.commit();
    assert_eq!(update.committed, "感じ");
    assert!(update.learning_error.is_some());
    assert!(!path.exists());
    drop(lock);
    type_text(&mut session, "Kanji");
    assert_eq!(session.process_key(Key::Space).preedit, "▼感じ");
    assert!(session.commit().learning_error.is_none());
}
#[test]
fn corrupt_or_unsupported_data_is_never_overwritten() {
    let directory = Directory::new();
    let path = directory.path();
    let (store, _) = UserDictionary::open(&path).unwrap();
    for data in ["broken", "abc /abbreviation/\n", "かな /(eval \"code\")/\n"] {
        fs::write(&path, data).unwrap();
        assert!(UserDictionary::open(&path).is_err());
        assert!(store.record(&choice("かな", "仮名", None)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), data);
    }
}
#[test]
fn explicit_store_lock_is_nonblocking() {
    let directory = Directory::new();
    let path = directory.path();
    let (store, _) = UserDictionary::open(&path).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.0.join("user.skk.lock"))
        .unwrap();
    lock.lock().unwrap();
    assert_eq!(
        store
            .record(&choice("かな", "仮名", None))
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
}
#[cfg(unix)]
#[test]
fn symlink_targets_are_updated_without_replacing_the_symlink() {
    let directory = Directory::new();
    let target = directory.0.join("target.skk");
    let link = directory.path();
    fs::write(&target, "").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let (store, _) = UserDictionary::open(&link).unwrap();
    store.record(&choice("かな", "仮名", None)).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        Dictionary::load(&target).unwrap().lookup("かな")[0].text,
        "仮名"
    );
    fs::remove_file(&target).unwrap();
    assert!(UserDictionary::open(&link).is_err());
}
