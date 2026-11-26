//! The portability of the core (§11.4).
//!
//! The golden hash is only a *cross-platform* baseline if the core computes the same
//! bits everywhere. The one place where two platforms can disagree is a transcendental
//! function: `sin`, `cos`, `exp` and the rest are not specified by IEEE-754, and the
//! platform's libm differs between glibc, musl, macOS and the hardware's SIMD paths.
//!
//! So the core uses the [`libm`](https://crates.io/crates/libm) crate - a pure-Rust
//! implementation, the same code on every target - and nothing else. This test checks
//! that, in the source, on every build: a platform `sin` that leaks into the core is
//! found here, on the machine that wrote it, instead of as a hash mismatch on the
//! platform that did not.
//!
//! The scan covers every `.rs` file under `src/`, at any depth, so a module moved into
//! a subdirectory cannot slip out of it. The core's own vector helpers (`normalize` and
//! the rest) live in that tree and are checked like everything else; they get away with
//! `sqrt` because IEEE-754 requires it to be correctly rounded. The ban list includes
//! the LLVM intrinsic `powi` (folded differently from `powf` at compile time) and the
//! transcendentals that are easy to miss: `atan2`, `asin`, `acos`, `tan`, `ln`, `cbrt`.

use std::fs;
use std::path::PathBuf;

/// The source directory of the core.
fn source_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The functions that are *not* IEEE-754 exact, and so must come from `libm`.
///
/// `sqrt` is not on the list: IEEE-754 requires it to be correctly rounded, so every
/// platform returns the same bits. The same goes for the basic arithmetic. `to_radians`
/// and `to_degrees` are one multiplication or division by a constant, so they are exact
/// too, in the sense that matters here.
const FORBIDDEN: &[&str] = &[
    ".sin(", ".cos(", ".tan(", ".asin(", ".acos(", ".atan(", ".atan2(", ".sinh(", ".cosh(",
    ".tanh(", ".exp(", ".exp2(", ".exp_m1(", ".ln(", ".ln_1p(", ".log(", ".log2(", ".log10(",
    ".powf(", ".powi(", ".cbrt(", ".hypot(",
];

/// The free-function spellings of the same calls, for both float widths.
///
/// The method spellings in [`FORBIDDEN`] cover `.sin()` on either width, but a
/// free function is written `f64::sin` / `f32::sin` and needs its own list.
/// `sin_cos`, `asinh`, `acosh` and `atanh` are easy to miss even on `f64`.
const FREE_FUNCTIONS: &[&str] = &[
    "f64::sin",
    "f64::cos",
    "f64::tan",
    "f64::asin",
    "f64::acos",
    "f64::atan",
    "f64::atan2",
    "f64::sinh",
    "f64::cosh",
    "f64::tanh",
    "f64::exp",
    "f64::exp2",
    "f64::exp_m1",
    "f64::ln",
    "f64::ln_1p",
    "f64::log",
    "f64::log2",
    "f64::log10",
    "f64::powf",
    "f64::powi",
    "f64::hypot",
    "f64::cbrt",
    "f64::sin_cos",
    "f64::asinh",
    "f64::acosh",
    "f64::atanh",
    "f32::sin",
    "f32::cos",
    "f32::tan",
    "f32::asin",
    "f32::acos",
    "f32::atan",
    "f32::atan2",
    "f32::sinh",
    "f32::cosh",
    "f32::tanh",
    "f32::exp",
    "f32::exp2",
    "f32::exp_m1",
    "f32::ln",
    "f32::ln_1p",
    "f32::log",
    "f32::log2",
    "f32::log10",
    "f32::powf",
    "f32::powi",
    "f32::hypot",
    "f32::cbrt",
    "f32::sin_cos",
    "f32::asinh",
    "f32::acosh",
    "f32::atanh",
];

/// Blanks the comments and literal contents of a source file, preserving
/// newlines. A doc comment *mentioning* `f64::sin`, or a string containing
/// `#[cfg(test)]`, must not be mistaken for code; a `//` or `/*` inside a
/// literal must not start a comment either.
fn strip_comments_and_literals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                out.push_str("  ");
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push(c);
                        break;
                    }
                    out.push(' ');
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                out.push_str("  ");
                let mut star = false;
                for c in chars.by_ref() {
                    if star && c == '/' {
                        break;
                    }
                    out.push(if c == '\n' { '\n' } else { ' ' });
                    star = c == '*';
                }
            }
            '"' => {
                // A normal string: blank it, honouring backslash escapes, so a
                // forbidden name or an attribute inside it cannot hide code.
                out.push(' ');
                let mut escaped = false;
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push(c);
                    } else if escaped {
                        out.push(' ');
                        escaped = false;
                    } else if c == '\\' {
                        out.push(' ');
                        escaped = true;
                    } else if c == '"' {
                        out.push(' ');
                        break;
                    } else {
                        out.push(' ');
                    }
                }
            }
            'r' if matches!(chars.peek(), Some(&'"') | Some(&'#')) => {
                // Raw strings (`r"…"`, `r#"…"#`, …). A raw identifier (`r#name`)
                // has no opening quote and must be left as ordinary code.
                let mut probe = chars.clone();
                let mut hashes = 0usize;
                while probe.peek() == Some(&'#') {
                    probe.next();
                    hashes += 1;
                }
                if probe.peek() == Some(&'"') {
                    out.push(' ');
                    for _ in 0..hashes {
                        chars.next();
                        out.push(' ');
                    }
                    chars.next(); // the opening quote
                    out.push(' ');
                    while let Some(ch) = chars.next() {
                        out.push(if ch == '\n' { '\n' } else { ' ' });
                        if ch == '"' {
                            let mut tail = chars.clone();
                            let mut seen = 0usize;
                            while seen < hashes && tail.peek() == Some(&'#') {
                                tail.next();
                                seen += 1;
                            }
                            if seen == hashes {
                                for _ in 0..hashes {
                                    chars.next();
                                    out.push(' ');
                                }
                                break;
                            }
                        }
                    }
                } else {
                    out.push(c);
                }
            }
            '\'' => {
                // A char literal (including `'"'`) is blanked whole so the
                // quote inside it is not taken for a string; a lifetime (`'a`)
                // is ordinary code and is copied as is.
                let is_char = match chars.peek() {
                    Some('\\') => true,
                    Some(_) => chars.clone().nth(1) == Some('\''),
                    None => false,
                };
                if is_char {
                    out.push(' ');
                    let mut escaped = false;
                    for c in chars.by_ref() {
                        out.push(if c == '\n' { '\n' } else { ' ' });
                        if escaped {
                            escaped = false;
                        } else if c == '\\' {
                            escaped = true;
                        } else if c == '\'' {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Every `.rs` file under `src/`, at any depth.
///
/// The scan is recursive on purpose: a module moved into a subdirectory (`src/math/…`)
/// must not slip out of the check, because the functions it may not call are the same
/// wherever the file lives.
fn source_files(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("the source directory") {
        let path = entry.expect("a file").path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn check_no_platform_transcendental(dir: PathBuf, minimum_files: usize, what: &str) {
    let mut files = Vec::new();
    source_files(&dir, &mut files);
    let mut checked = 0;
    for path in files {
        let text = fs::read_to_string(&path).expect("a source file");
        // Strip first, then cut at `#[cfg(test)]`: an attribute in a comment or a
        // string is not a test module and must not exempt the code after it.
        // Real test modules are allowed to use the platform's functions: they are
        // not part of the simulation, and a test that reads an angle in degrees
        // wants the platform's `to_radians`.
        let stripped = strip_comments_and_literals(&text);
        let code = match stripped.find("#[cfg(test)]") {
            Some(index) => &stripped[..index],
            None => &stripped[..],
        };
        for function in FORBIDDEN {
            assert!(
                !code.contains(function),
                "{} calls the platform's `{function}`: {what} must use `libm` (§11.4)",
                path.display()
            );
        }
        // The same for the free functions, `f64::sin` / `f32::sin` and the rest.
        for function in FREE_FUNCTIONS {
            assert!(
                !code.contains(function),
                "{} calls the platform's `{function}`: {what} must use `libm` (§11.4)",
                path.display()
            );
        }
        checked += 1;
    }
    assert!(
        checked >= minimum_files,
        "only {checked} source files were checked"
    );
}

#[test]
fn an_attribute_in_a_comment_or_string_does_not_hide_later_code() {
    let text = r#"
        // #[cfg(test)]
        let quote = '"';
        let s = "// #[cfg(test)]";
        let x = f64::sinh(1.0);
        #[cfg(test)]
        mod tests {}
    "#;
    let stripped = strip_comments_and_literals(text);
    let code = match stripped.find("#[cfg(test)]") {
        Some(index) => &stripped[..index],
        None => &stripped[..],
    };
    assert!(!code.contains("#[cfg(test)]"));
    assert!(code.contains("f64::sinh"));
}

#[test]
fn the_core_calls_no_platform_transcendental_function() {
    check_no_platform_transcendental(source_dir(), 10, "the core");
}

#[test]
fn the_shared_helper_calls_no_platform_transcendental_function() {
    // pool-intent builds the aim direction for both the bridge and the server.
    // A platform `sin` there would drift exactly where the canary is supposed
    // to catch drift, so it is held to the same rule as the core.
    let intent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("pool-intent")
        .join("src");
    check_no_platform_transcendental(intent, 4, "pool-intent");
}

#[test]
fn the_libm_version_is_pinned() {
    // The `libm` crate is the core's only source of transcendental functions, so its
    // version is part of the golden value: a different `libm` is a different hash.
    // The lock file is what pins it, and it is in the repository.
    let lock = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Cargo.lock");
    let text = fs::read_to_string(lock).expect("the lock file");
    let version = text
        .split("name = \"libm\"")
        .nth(1)
        .and_then(|rest| rest.split("version = \"").nth(1))
        .and_then(|rest| rest.split('"').next())
        .expect("the libm version in the lock file");
    assert!(
        !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.'),
        "the libm version in the lock file is not a version: {version}"
    );
}
