//! `fogline version`.

use std::io::{self, Write};

use serde::Serialize;

/// What `fogline version --json` prints. Field order is the output order.
#[derive(Serialize)]
struct Report<'a> {
    name: &'a str,
    version: &'a str,
    /// The platform this binary was built for, as a Rust target triple.
    target: &'a str,
    /// The commit a release was built from. Absent in local builds.
    commit: Option<&'a str>,
    /// The git found on this machine, or null if there is none. The scan
    /// depends on it, so this is the first thing to check when a scan fails.
    git: Option<String>,
}

/// Writes the plain form (`fogline 0.1.0`) or one line of JSON.
pub fn write(out: &mut impl Write, json: bool) -> io::Result<()> {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    if !json {
        return writeln!(out, "{name} {version}");
    }
    let report = Report {
        name,
        version,
        target: env!("FOGLINE_TARGET"),
        commit: option_env!("FOGLINE_BUILD_COMMIT"),
        git: fogline_core::git::detect_version()
            .ok()
            .map(|found| found.to_string()),
    };
    serde_json::to_writer(&mut *out, &report)?;
    writeln!(out)
}

#[cfg(test)]
mod tests {
    use super::write;

    fn output(json: bool) -> String {
        let mut buffer = Vec::new();
        write(&mut buffer, json).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn the_plain_form_is_name_then_version() {
        assert_eq!(
            output(false),
            format!("fogline {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn the_json_form_keeps_its_field_order() {
        let text = output(true);
        let positions: Vec<usize> = [
            "\"name\"",
            "\"version\"",
            "\"target\"",
            "\"commit\"",
            "\"git\"",
        ]
        .iter()
        .map(|field| text.find(field).unwrap())
        .collect();
        assert!(positions.is_sorted(), "fields out of order in {text}");
        assert!(text.ends_with("}\n"));
    }
}
