//! `goldens.lock`: the cloudspec commit the goldens come from and the sha256 of each golden file.

use std::path::Path;

use weft_cloudspec::digest;

use crate::{fixtures, root, scratch};

/// The commit of cloudspec the goldens are taken from.
const COMMIT: &str = "f57337466c7c6c73863bbdc8bd02bedafc8f913d";

/// The lock of `dir`: its commit, and `(sha256 hex, file)` per golden in the lock's order.
fn lock(dir: &Path) -> (String, Vec<(String, String)>) {
    let path = dir.join("goldens.lock");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines();
    let commit = lines
        .next()
        .and_then(|l| l.strip_prefix("commit "))
        .expect("the first line of goldens.lock is `commit <sha>`")
        .to_owned();
    let files = lines
        .map(|l| {
            let (hex, file) = l
                .split_once(' ')
                .unwrap_or_else(|| panic!("goldens.lock: `{l}` is not `<sha256> <file>`"));
            (hex.to_owned(), file.to_owned())
        })
        .collect();
    (commit, files)
}

/// The files of `dir` whose sha256 differs from their line of the lock, in the lock's order.
fn mismatches(dir: &Path) -> Vec<String> {
    lock(dir)
        .1
        .into_iter()
        .filter(|(hex, file)| {
            let bytes = std::fs::read(dir.join(file)).unwrap_or_default();
            digest(&bytes) != format!("sha256:{hex}")
        })
        .map(|(_, file)| file)
        .collect()
}

/// `CLOUDSPEC_REV` of `Taskfile.yml` in `root`: the one pin of the vendored blocks and of
/// cloudspec-sim.
fn pin(root: &Path) -> String {
    let path = root.join("Taskfile.yml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .find_map(|l| l.strip_prefix("  CLOUDSPEC_REV: "))
        .expect("Taskfile.yml sets CLOUDSPEC_REV")
        .trim()
        .to_owned()
}

/// `Err` naming both commits when the lock of `dir` names another commit than `rev`.
fn lock_at_pin(dir: &Path, rev: &str) -> Result<(), String> {
    let (commit, _) = lock(dir);
    if commit == rev {
        Ok(())
    } else {
        Err(format!(
            "goldens.lock names {commit}, CLOUDSPEC_REV is {rev}"
        ))
    }
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn the_lock_names_cloudspecs_commit_and_each_golden_once() {
    let (commit, files) = lock(&fixtures());
    assert_eq!(commit, COMMIT);
    let names: Vec<&str> = files.iter().map(|(_, f)| f.as_str()).collect();
    assert_eq!(
        names,
        [
            "snippet.submission.json",
            "workstation.submission.json",
            "host-daemon.submission.json"
        ]
    );
    for (hex, file) in &files {
        assert!(is_hex(hex, 64), "{file}: `{hex}` is not a sha256 in hex");
    }
    let mut on_disk: Vec<String> = std::fs::read_dir(fixtures())
        .expect("the fixtures directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n != "goldens.lock")
        .collect();
    on_disk.sort();
    let mut locked: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    locked.sort();
    assert_eq!(on_disk, locked, "every file beside goldens.lock is locked");
}

#[test]
fn each_golden_matches_its_lock_line() {
    assert_eq!(mismatches(&fixtures()), Vec::<String>::new());
    let (_, files) = lock(&fixtures());
    let expected = [
        "d58f6b6cf43d2f0bb672a11c68c7bb51c0b848dbddf2b9ff999ee08567dfaead",
        "ee10ee247b875d03f2d6ec520f833abe3cc0657aa3dbe3e53af372f0154e5245",
        "25303268d8346b841fe1f97a8a30a5b996d006e8dae0dbf9426d86bcaa5201c4",
    ];
    let hexes: Vec<&str> = files.iter().map(|(h, _)| h.as_str()).collect();
    assert_eq!(hexes, expected);
}

#[test]
fn one_changed_byte_of_a_copy_in_target_is_a_mismatch() {
    let dir = scratch("one-changed-byte");
    for entry in std::fs::read_dir(fixtures()).expect("the fixtures directory") {
        let entry = entry.expect("an entry");
        std::fs::copy(entry.path(), dir.join(entry.file_name())).expect("a copy");
    }
    assert_eq!(mismatches(&dir), Vec::<String>::new(), "the copy matches");
    let path = dir.join("workstation.submission.json");
    let mut bytes = std::fs::read(&path).expect("the copy");
    let middle = bytes.len() / 2;
    bytes[middle] ^= 1;
    std::fs::write(&path, &bytes).expect("the changed copy");
    assert_eq!(mismatches(&dir), ["workstation.submission.json"]);
}

/// The goldens and the vendored blocks come from one cloudspec commit: the lock names
/// `CLOUDSPEC_REV`, so a pin move that leaves the goldens behind fails here.
#[test]
fn the_lock_names_the_cloudspec_rev_of_the_taskfile() {
    let rev = pin(&root());
    assert!(is_hex(&rev, 40), "CLOUDSPEC_REV `{rev}` is not a commit");
    assert_eq!(lock_at_pin(&fixtures(), &rev), Ok(()));
}

/// A copy of the lock that names another commit than the pin is refused, naming both.
#[test]
fn a_lock_naming_another_commit_than_the_pin_is_refused() {
    let dir = scratch("lock-of-another-commit");
    let text = std::fs::read_to_string(fixtures().join("goldens.lock")).expect("the lock");
    let (_, rest) = text.split_once('\n').expect("a lock of more than one line");
    let other = "a485b7f459abd66261af25b558fb92df90570545";
    std::fs::write(dir.join("goldens.lock"), format!("commit {other}\n{rest}"))
        .expect("the changed lock");
    let rev = pin(&root());
    assert_ne!(rev, other, "the pin has moved past {other}");
    assert_eq!(
        lock_at_pin(&dir, &rev),
        Err(format!(
            "goldens.lock names {other}, CLOUDSPEC_REV is {rev}"
        ))
    );
    assert_eq!(lock_at_pin(&dir, other), Ok(()));
}
