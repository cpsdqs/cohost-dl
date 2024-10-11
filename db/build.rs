use std::io;
use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .and_then(|out| {
            if !out.status.success() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "error: exited with {:?}\n{}",
                        out.status,
                        String::from_utf8_lossy(&out.stderr)
                    ),
                ));
            }

            String::from_utf8(out.stdout).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        })
        .expect("could not determine commit hash");

    println!("cargo:rustc-env=BUILD_COMMIT={hash}");
}
