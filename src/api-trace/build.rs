//! Stamps the commit this build came from into the binary.
//!
//! A log we are sent is a log from a build we no longer have in front of us.
//! Without the commit on every line, the first thing we have to ask is which
//! version produced it, and that is a question the log should answer itself.

use std::process::Command;

fn main() {
    // The commit is read at build time, so the build has to be redone when it
    // changes. Both paths matter: `HEAD` for switching branches, the file it
    // points at for committing on one.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");

    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
        // A build from a tarball, or from anywhere without git, still logs. A
        // line saying it does not know the commit beats no line at all.
        .unwrap_or_else(|| String::from("unknown"));

    println!("cargo:rustc-env=MERIDIAN_SHA={sha}");
}
