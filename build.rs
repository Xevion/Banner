use std::process::Command;

fn main() {
    // rust-embed requires the folder to exist at compile time, even when empty.
    // Create it so the backend compiles without a prior frontend build.
    let embed_dir = std::path::Path::new("web/build/client");
    if !embed_dir.exists() {
        std::fs::create_dir_all(embed_dir).expect("failed to create web/build/client");
    }
    println!("cargo:rerun-if-changed=web/build/client");

    // Try to get Git commit hash from Railway environment variable first
    let git_hash = std::env::var("RAILWAY_GIT_COMMIT_SHA").unwrap_or_else(|_| {
        // Fallback to git command if not on Railway
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
