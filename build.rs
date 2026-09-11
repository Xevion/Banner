use std::process::Command;

fn main() {
    // The other rerun-if directives opt out of the default "rerun on any change", so without this
    // a cached target dir reuses the previous deploy's hash and /api/status reports the wrong one.
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHA");

    // Prefer an explicitly supplied commit: the deploy rsyncs without .git, so the git fallback
    // below has no repository to read and would otherwise report "unknown" in /api/status.
    let git_hash = std::env::var("GIT_COMMIT_SHA").unwrap_or_else(|_| {
        // Fallback for local builds, where the repository is present.
        let output = Command::new("git").args(["rev-parse", "HEAD"]).output();
        match output {
            Ok(output) => {
                if output.status.success() {
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                } else {
                    "unknown".to_string()
                }
            }
            Err(_) => "unknown".to_string(),
        }
    });

    // Get the short hash (first 7 characters)
    let short_hash = if git_hash != "unknown" && git_hash.len() >= 7 {
        git_hash[..7].to_string()
    } else {
        git_hash.clone()
    };

    // Set the environment variables that will be available at compile time
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_COMMIT_SHORT={}", short_hash);

    // Rebuild if the Git commit changes (only works when .git directory is available)
    if std::path::Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/refs/heads");
    }
}
