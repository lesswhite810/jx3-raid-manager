use std::path::{Path, PathBuf};

#[cfg(test)]
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Installer,
    Portable,
}

pub fn current_executable_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|err| format!("获取当前程序路径失败: {err}"))
}

pub fn current_executable_dir() -> Result<PathBuf, String> {
    current_executable_path()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法识别当前程序目录".to_string())
}

pub fn executable_has_uninstall_marker(executable_path: &Path) -> bool {
    executable_path
        .parent()
        .map(|dir| dir.join("uninstall.exe").exists())
        .unwrap_or(false)
}

pub fn executable_has_portable_marker(executable_path: &Path) -> bool {
    executable_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.contains("_v"))
        .unwrap_or(false)
}

pub fn detect_runtime_mode_for_executable(executable_path: &Path) -> RuntimeMode {
    if executable_has_portable_marker(executable_path)
        || !executable_has_uninstall_marker(executable_path)
    {
        RuntimeMode::Portable
    } else {
        RuntimeMode::Installer
    }
}

pub fn detect_current_runtime_mode() -> Result<RuntimeMode, String> {
    Ok(detect_runtime_mode_for_executable(&current_executable_path()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestTempDir;

    #[test]
    fn treats_versioned_release_executable_as_portable_even_if_uninstall_exists() {
        let temp_dir = TestTempDir::new("jx3-runtime-mode-test");
        let exe_path = temp_dir.join("JX3RaidManager_v2.1.18.exe");
        fs::write(temp_dir.join("uninstall.exe"), "").expect("uninstall marker should exist");

        assert_eq!(
            detect_runtime_mode_for_executable(&exe_path),
            RuntimeMode::Portable
        );
    }

    #[test]
    fn treats_unversioned_executable_with_uninstall_marker_as_installer() {
        let temp_dir = TestTempDir::new("jx3-runtime-mode-test");
        let exe_path = temp_dir.join("JX3RaidManager.exe");
        fs::write(temp_dir.join("uninstall.exe"), "").expect("uninstall marker should exist");

        assert_eq!(
            detect_runtime_mode_for_executable(&exe_path),
            RuntimeMode::Installer
        );
    }

    #[test]
    fn treats_directory_without_uninstall_marker_as_portable() {
        let temp_dir = TestTempDir::new("jx3-runtime-mode-test");
        let exe_path = temp_dir.join("JX3RaidManager.exe");

        assert_eq!(
            detect_runtime_mode_for_executable(&exe_path),
            RuntimeMode::Portable
        );
    }
}
