// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
//! Writes the generated reference pages (`notaio::reference`) into a directory.
//!
//! `docgen <dir>`: `make docs-reference` writes `docs/src/reference/`, and
//! `make docs-reference-check` writes a temporary directory and fails CI on any difference.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: docgen <dir>");
        return ExitCode::from(2);
    };
    for (name, content) in notaio::reference::pages() {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, content) {
            eprintln!("{}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
