use crate::error::UpdateError;
use sha2::{Digest, Sha256};
use std::io::Write;

/// The lines of a `SHA256SUMS.txt`: `<64 hex digits>  <file name>` (a `*` before the name marks
/// binary mode and is ignored).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sums {
    entries: Vec<(String, String)>,
}

impl Sums {
    /// The fingerprint listed for `name`, lower case.
    pub fn of(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, h)| h.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Reads a sums file. Lines that are not `hash  name` are ignored, so a comment or a blank line
/// does no harm; a hash that is not 64 hex digits is ignored too, so it can never match.
pub fn parse_sums(text: &str) -> Sums {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((hash, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let name = rest.trim().trim_start_matches('*');
        let hash = hash.to_ascii_lowercase();
        if hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !name.is_empty() {
            entries.push((name.to_string(), hash));
        }
    }
    Sums { entries }
}

/// A writer that fingerprints what passes through it.
pub(crate) struct HashingWriter<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> HashingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    /// Flush, and return the file and the fingerprint (lower-case hex).
    pub fn finish(mut self) -> Result<(W, String), UpdateError> {
        self.inner
            .flush()
            .map_err(|e| UpdateError::Disk(e.to_string()))?;
        let digest = self.hasher.finalize();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        Ok((self.inner, hex))
    }
}

impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const HASH_B: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn reads_the_usual_layout() {
        let text = format!("{HASH_A}  CargoFrame-0.2.0-windows-x64-setup.exe\n{HASH_B} *CargoFrame-0.2.0-linux-x64.tar.gz\n");
        let sums = parse_sums(&text);
        assert_eq!(sums.len(), 2);
        assert_eq!(
            sums.of("CargoFrame-0.2.0-windows-x64-setup.exe"),
            Some(HASH_A)
        );
        assert_eq!(sums.of("CargoFrame-0.2.0-linux-x64.tar.gz"), Some(HASH_B));
        assert_eq!(sums.of("other"), None);
    }

    #[test]
    fn upper_case_and_windows_line_endings_are_fine() {
        let text = format!("{}  a.exe\r\n", HASH_A.to_ascii_uppercase());
        assert_eq!(parse_sums(&text).of("a.exe"), Some(HASH_A));
    }

    #[test]
    fn junk_lines_and_short_hashes_are_ignored() {
        let text = format!("# comment\n\nabc123  short.exe\nnot-a-hash-at-all-not-a-hash-at-all-not-a-hash-at-all-not-a-hash-zz  bad.exe\n{HASH_A}\n{HASH_B}  ok.exe\n");
        let sums = parse_sums(&text);
        assert_eq!(sums.len(), 1);
        assert_eq!(sums.of("ok.exe"), Some(HASH_B));
    }

    #[test]
    fn the_first_entry_for_a_name_wins() {
        let text = format!("{HASH_A}  x\n{HASH_B}  x\n");
        assert_eq!(parse_sums(&text).of("x"), Some(HASH_A));
    }

    #[test]
    fn a_hashing_writer_fingerprints_what_it_writes() {
        let mut w = HashingWriter::new(Vec::new());
        w.write_all(b"abc").unwrap();
        let (bytes, hash) = w.finish().unwrap();
        assert_eq!(bytes, b"abc");
        assert_eq!(hash, HASH_B);
    }

    #[test]
    fn an_empty_input_has_the_known_empty_fingerprint() {
        let (_, hash) = HashingWriter::new(Vec::new()).finish().unwrap();
        assert_eq!(hash, HASH_A);
    }
}
