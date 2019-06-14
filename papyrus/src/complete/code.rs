//! Completion for rust source code using [`racer`].
//!
//! [`racer`]: racer
use super::*;
use std::io;
use std::path::Path;

use racer::{BytePos, FileCache, Location, Match};

const LIBRS: &'static str = "lib.rs";

/// Completion used for rust code in the repl.
pub struct CodeCompleter {
    last_code: String,
    split: std::ops::Range<usize>,
}

impl CodeCompleter {
    /// Build the code completion state. Uses the current repl state.
    pub fn build<T>(repl_data: &crate::repl::ReplData<T>) -> Self {
        let (last_code, map) =
            crate::pfh::code::construct_source_code(repl_data.file_map(), repl_data.linking());

        let split = map.get(repl_data.current_file()).cloned().unwrap_or(0..0); // return an empty range if this fails

        CodeCompleter { last_code, split }
    }

    /// Returns the start position of the _last_ word which is broken, in context to rust code.
    pub fn word_break(line: &str) -> usize {
        word_break_start(line, &[' ', ':', '.'])
    }

    /// Get completions that would match a string injected into the current repl state.
    pub fn complete(&self, injection: &str, limit: Option<usize>, cache: &CodeCache) -> Vec<Match> {
        let limit = limit.unwrap_or(std::usize::MAX);

        let session = racer::Session::new(&cache.cache);

        let (contents, pos) = self.inject(injection);

        session.cache_file_contents(LIBRS, contents);

        racer::complete_from_file(LIBRS, Location::Point(pos), &session)
            .take(limit)
            .collect()
    }

    /// Inject code into the current source code and return the amended code,
    /// along with the byte position to complete from.
    pub fn inject(&self, injection: &str) -> (String, BytePos) {
        let cap = self.last_code.len() + self.split.start - self.split.end + injection.len();
        let mut s = String::with_capacity(cap);

        s.push_str(&self.last_code[..self.split.start]);
        s.push_str(injection);
        s.push_str(&self.last_code[self.split.end..]);

        debug_assert_eq!(s.len(), cap);

        let pos = (self.split.start + injection.len()).into();

        (s, pos)
    }
}

/// Caching for code.
pub struct CodeCache {
    cache: FileCache,
}

impl CodeCache {
	/// Construct new cache.
    pub fn new() -> Self {
        Self {
            cache: FileCache::new(PapyrusCodeFileLoader),
        }
    }
}

struct PapyrusCodeFileLoader;

impl racer::FileLoader for PapyrusCodeFileLoader {
    fn load_file(&self, path: &Path) -> io::Result<String> {
        use std::fs::File;
        use std::io::Read;

        // copied from racers implementation and special handling for lib.rs

        if path == Path::new(LIBRS) {
            Ok(String::new())
        } else {
            let mut rawbytes = Vec::new();
            let mut f = File::open(path)?;
            f.read_to_end(&mut rawbytes)?;

            // skip BOM bytes, if present
            if rawbytes.len() > 2 && rawbytes[0..3] == [0xEF, 0xBB, 0xBF] {
                std::str::from_utf8(&rawbytes[3..])
                    .map(|s| s.to_owned())
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            } else {
                String::from_utf8(rawbytes).map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_test() {
        let cc = CodeCompleter {
            last_code: String::from("Hello morld"),
            split: 5..7, // cut out ' m' such that "Hello" and "orld" is it
        };

        let (s, pos) = cc.inject(", w");

        assert_eq!(&s, "Hello, world");
        assert_eq!(pos, BytePos(8));

        let cc = CodeCompleter {
            last_code: String::from("Hello"),
            split: 5..5, // inject to end
        };

        let (s, pos) = cc.inject(", world");

        assert_eq!(&s, "Hello, world");
        assert_eq!(pos, BytePos(12));

        let cc = CodeCompleter {
            last_code: String::from(", world"),
            split: 0..0, // inject at start
        };

        let (s, pos) = cc.inject("Hello");

        assert_eq!(&s, "Hello, world");
        assert_eq!(pos, BytePos(5));

        let cc = CodeCompleter {
            last_code: String::from("Hello, worm"),
            split: 10..11, // cut less than added
        };

        let (s, pos) = cc.inject("ld");

        assert_eq!(&s, "Hello, world");
        assert_eq!(pos, BytePos(12));
    }

    #[test]
    fn complete_test() {
        let cc = CodeCompleter {
            last_code: String::from("fn apple() {} \n\n fn main() {  }"),
            split: 29..29,
        };

        let (s, _) = cc.inject("ap");

        assert_eq!(&s, "fn apple() {} \n\n fn main() { ap }");

        let matches = cc.complete("ap", None);

        assert_eq!(matches.get(0).map(|x| x.matchstr.as_str()), Some("apple"));
    }
}
