use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, Window};
use tauri_plugin_updater::UpdaterExt;
use url::Url;

const GITEE_REPO: &str = "lesswhite/jx3-raid-manager";
const GITHUB_UPDATER_ENDPOINT: &str =
    "https://github.com/lesswhite810/jx3-raid-manager/releases/latest/download/latest.json";
const GITEE_UPDATER_ENDPOINT: &str =
    "https://gitee.com/lesswhite/jx3-raid-manager/raw/updater-assets/updater/latest.json";
const UPDATER_PROGRESS_EVENT: &str = "updater://progress";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterRuntimeInfo {
    current_version: String,
    executable_path: String,
    is_portable: bool,
    will_install_in_place: bool,
    has_uninstall_executable: bool,
    updater_configured: bool,
    release_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterCheckResult {
    current_version: String,
    available: bool,
    version: Option<String>,
    body: Option<String>,
    pub_date: Option<String>,
    is_portable: bool,
    will_install_in_place: bool,
    updater_configured: bool,
    release_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdaterProgressPayload {
    event: String,
    content_length: Option<u64>,
    chunk_length: Option<usize>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
}

fn updater_pubkey() -> String {
    option_env!("TAURI_PUBLIC_KEY")
        .unwrap_or("")
        .trim()
        .to_string()
}

fn updater_release_url(tag: Option<&str>) -> String {
    match tag {
        Some(tag) if !tag.trim().is_empty() => {
            format!("https://gitee.com/{GITEE_REPO}/releases/tag/{tag}")
        }
        _ => format!("https://gitee.com/{GITEE_REPO}/releases"),
    }
}

fn detect_runtime_info<R: Runtime>(app: &AppHandle<R>) -> Result<UpdaterRuntimeInfo, String> {
    let current_exe =
        std::env::current_exe().map_err(|err| format!("获取当前程序路径失败: {err}"))?;
    let executable_path = current_exe.to_string_lossy().to_string();
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| "无法识别当前程序目录".to_string())?;
    let has_uninstall_executable = install_dir.join("uninstall.exe").exists();
    let looks_like_portable = current_exe
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.contains("_v"))
        .unwrap_or(false);
    let is_portable = looks_like_portable || !has_uninstall_executable;

    Ok(UpdaterRuntimeInfo {
        current_version: app.package_info().version.to_string(),
        executable_path,
        is_portable,
        will_install_in_place: !is_portable,
        has_uninstall_executable,
        updater_configured: !updater_pubkey().is_empty(),
        release_url: updater_release_url(None),
    })
}

fn build_updater<R: Runtime>(app: &AppHandle<R>) -> Result<tauri_plugin_updater::Updater, String> {
    let pubkey = updater_pubkey();
    if pubkey.is_empty() {
        return Err("当前构建未启用自动更新".to_string());
    }

    let github_endpoint =
        Url::parse(GITHUB_UPDATER_ENDPOINT).map_err(|err| format!("GitHub 更新地址无效: {err}"))?;
    let gitee_endpoint =
        Url::parse(GITEE_UPDATER_ENDPOINT).map_err(|err| format!("Gitee 更新地址无效: {err}"))?;

    app.updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![github_endpoint, gitee_endpoint])
        .map_err(|err| format!("配置更新服务失败: {err}"))?
        .build()
        .map_err(|err| format!("初始化更新服务失败: {err}"))
}

#[tauri::command]
pub fn updater_get_runtime_info(app: AppHandle) -> Result<UpdaterRuntimeInfo, String> {
    detect_runtime_info(&app)
}

#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Result<UpdaterCheckResult, String> {
    let runtime_info = detect_runtime_info(&app)?;
    if !runtime_info.updater_configured {
        return Ok(UpdaterCheckResult {
            current_version: runtime_info.current_version,
            available: false,
            version: None,
            body: None,
            pub_date: None,
            is_portable: runtime_info.is_portable,
            will_install_in_place: runtime_info.will_install_in_place,
            updater_configured: false,
            release_url: runtime_info.release_url,
        });
    }

    let updater = build_updater(&app)?;
    let maybe_update = updater
        .check()
        .await
        .map_err(|err| format!("检查更新失败: {err}"))?;

    if let Some(update) = maybe_update {
        let release_url = updater_release_url(Some(&format!("v{}", update.version)));
        Ok(UpdaterCheckResult {
            current_version: update.current_version.clone(),
            available: true,
            version: Some(update.version.clone()),
            body: update.body.clone(),
            pub_date: update.date.map(|value| value.to_string()),
            is_portable: runtime_info.is_portable,
            will_install_in_place: runtime_info.will_install_in_place,
            updater_configured: true,
            release_url,
        })
    } else {
        Ok(UpdaterCheckResult {
            current_version: runtime_info.current_version,
            available: false,
            version: None,
            body: None,
            pub_date: None,
            is_portable: runtime_info.is_portable,
            will_install_in_place: runtime_info.will_install_in_place,
            updater_configured: true,
            release_url: runtime_info.release_url,
        })
    }
}

#[tauri::command]
pub async fn updater_download_and_install(app: AppHandle, window: Window) -> Result<(), String> {
    let runtime_info = detect_runtime_info(&app)?;
    if runtime_info.is_portable {
        return Err("便携版不支持应用内自动更新，请前往 Gitee 或 GitHub Release 下载新版本".to_string());
    }

    if !runtime_info.updater_configured {
        return Err("当前构建未启用自动更新".to_string());
    }

    let updater = build_updater(&app)?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|err| format!("重新检查更新失败: {err}"))?
    else {
        return Err("当前已是最新版本".to_string());
    };

    let mut downloaded_bytes: u64 = 0;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded_bytes += chunk_length as u64;
                let event = if downloaded_bytes == chunk_length as u64 {
                    UpdaterProgressPayload {
                        event: "started".to_string(),
                        content_length,
                        chunk_length: Some(chunk_length),
                        downloaded_bytes: Some(downloaded_bytes),
                        total_bytes: content_length,
                    }
                } else {
                    UpdaterProgressPayload {
                        event: "progress".to_string(),
                        content_length,
                        chunk_length: Some(chunk_length),
                        downloaded_bytes: Some(downloaded_bytes),
                        total_bytes: content_length,
                    }
                };
                let _ = window.emit(UPDATER_PROGRESS_EVENT, event);
            },
            || {
                let _ = window.emit(
                    UPDATER_PROGRESS_EVENT,
                    UpdaterProgressPayload {
                        event: "finished".to_string(),
                        content_length: None,
                        chunk_length: None,
                        downloaded_bytes: None,
                        total_bytes: None,
                    },
                );
            },
        )
        .await
        .map_err(|err| format!("下载安装更新失败: {err}"))?;

    Ok(())
}
