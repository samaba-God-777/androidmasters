//! Finds `adb` / `fastboot` from Android SDK Platform-Tools.

use std::path::{Path, PathBuf};

pub fn executable_name(tool: &str) -> String {
    if cfg!(windows) { format!("{tool}.exe") } else { tool.to_string() }
}

/// Folders searched in order: configured folder, ANDROID_HOME / ANDROID_SDK_ROOT, standard SDK and
/// package-manager locations, then PATH.
pub fn candidate_dirs(configured: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = configured {
        dirs.push(dir.to_path_buf());
    }
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(v) = std::env::var_os(var) {
            dirs.push(PathBuf::from(v).join("platform-tools"));
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") {
        dirs.push(home.join("Library/Android/sdk/platform-tools"));
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    } else if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(local.join("Android\\Sdk\\platform-tools"));
            dirs.push(local.join("Microsoft\\WinGet\\Links"));
        }
        dirs.push(PathBuf::from("C:\\platform-tools"));
    } else {
        dirs.push(home.join("Android/Sdk/platform-tools"));
        dirs.push(PathBuf::from("/usr/bin"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

pub fn find(tool: &str, configured: Option<&Path>) -> Option<PathBuf> {
    let name = executable_name(tool);
    candidate_dirs(configured).into_iter().map(|d| d.join(&name)).find(|p| is_executable(p))
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn configured_folder_wins() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("adb");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find("adb", Some(dir.path())), Some(exe));
        let not_exec = tempfile::tempdir().unwrap();
        std::fs::write(not_exec.path().join("adb"), "").unwrap();
        assert_ne!(find("adb", Some(not_exec.path())), Some(not_exec.path().join("adb")));
    }
}
