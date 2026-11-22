//! 测试期临时文件/临时目录的统一清理设施。
//!
//! 本模块只在测试构建（`cargo test`）中编译，不参与发布产物。
//!
//! 设计目标：
//! 1. 测试产生的临时文件与目录都在守卫（guard）离开作用域时自动删除，
//!    不再依赖每个用例手写 `fs::remove_dir_all` / `fs::remove_file`；
//! 2. Windows 上删除失败通常是因为 SQLite 句柄或索引服务仍持有文件，
//!    因此删除带短暂重试；最终仍失败只记录告警，绝不让清理逻辑把测试判定为失败；
//! 3. 进程级共享目录（`db::get_home_app_dir` 等在测试构建下使用的固定目录）
//!    借助 `atexit` 在测试进程退出时统一清理。
#![allow(dead_code)]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 删除重试次数：Windows 上文件句柄回收有几毫秒延迟，少量重试即可覆盖。
const REMOVE_ATTEMPTS: usize = 5;
/// 每次重试之间的等待时间，取需求区间（20~50ms）的中间值。
const REMOVE_RETRY_DELAY: Duration = Duration::from_millis(25);

/// 生成一个进程内唯一、且带语义前缀的临时路径。
///
/// 前缀 + 进程号 + 单调时间戳 + 进程内自增序号，保证同一进程内并发测试也不会撞名。
pub(crate) fn unique_temp_path(prefix: &str) -> PathBuf {
    static SEQ: Mutex<u64> = Mutex::new(0);

    let seq = match SEQ.lock() {
        Ok(mut guard) => {
            *guard += 1;
            *guard
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            *guard += 1;
            *guard
        }
    };

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);

    std::env::temp_dir().join(format!(
        "{}-{}-{}-{}",
        prefix,
        std::process::id(),
        nanos,
        seq
    ))
}

fn remove_with_retry(path: &Path, remove: impl Fn(&Path) -> std::io::Result<()>) {
    for attempt in 0..REMOVE_ATTEMPTS {
        match remove(path) {
            Ok(()) => return,
            // 已被删除或从未存在都算清理成功。
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                if attempt + 1 == REMOVE_ATTEMPTS {
                    log::warn!(
                        "测试临时路径清理失败（已重试 {} 次）: {} - {}",
                        REMOVE_ATTEMPTS,
                        path.display(),
                        error
                    );
                    return;
                }
                std::thread::sleep(REMOVE_RETRY_DELAY);
            }
        }
    }
}

/// 带重试地递归删除目录；不存在视为成功，最终失败只记录告警。
pub(crate) fn remove_dir_all_with_retry(path: &Path) {
    remove_with_retry(path, |target| std::fs::remove_dir_all(target));
}

/// 带重试地删除单个文件；不存在视为成功，最终失败只记录告警。
pub(crate) fn remove_file_with_retry(path: &Path) {
    remove_with_retry(path, |target| std::fs::remove_file(target));
}

/// 测试期临时目录守卫：离开作用域时递归删除整个目录。
///
/// 通过 `Deref<Target = PathBuf>` 兼容既有写法：`&guard` 仍可传给它原本接收
/// `&Path` / `&PathBuf` 的函数，`guard.join(..)`、`guard.display()` 等方法照旧可用。
pub(crate) struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    /// 在系统临时目录下创建唯一子目录，并立即创建该目录。
    pub(crate) fn new(prefix: &str) -> Self {
        Self::from_path(unique_temp_path(prefix))
    }

    /// 接管一个已确定的临时路径（例如必须与生产代码算出的路径保持一致时）。
    pub(crate) fn from_path(path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).expect("测试临时目录应可创建");
        Self { path }
    }

    /// 目录本身。
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// 目录内的子路径（只拼接，不创建）。
    pub(crate) fn child(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.path.join(relative.as_ref())
    }
}

impl Deref for TestTempDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for TestTempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        remove_dir_all_with_retry(&self.path);
    }
}

/// 测试期临时文件守卫：离开作用域时删除该文件（不删除父目录）。
///
/// 用于必须直接落在临时目录根下、不能独占子目录的场景（例如沿用生产代码算出的文件名）。
pub(crate) struct TestTempFile {
    path: PathBuf,
}

impl TestTempFile {
    /// 在临时目录下生成唯一文件名；`extension` 不含点，例如 `"sqlite"`。
    pub(crate) fn new(prefix: &str, extension: &str) -> Self {
        let path = unique_temp_path(prefix).with_extension(extension);
        Self { path }
    }

    /// 接管一个已确定的临时文件路径（不创建文件）。
    pub(crate) fn from_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Deref for TestTempFile {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for TestTempFile {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempFile {
    fn drop(&mut self) {
        remove_file_with_retry(&self.path);
    }
}

/// 进程退出时需要「空目录就删掉」的共享目录。
///
/// 这些目录被同一测试进程中的多个用例复用，无法用用例级守卫回收；
/// 但它们可能同时承载用户预置的夹具（例如升级测试数据库），
/// 因此只在退出时确认其为空才删除，绝不误删夹具。
static SHARED_TEMP_DIRS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static ATEXIT_REGISTERED: Once = Once::new();

extern "C" {
    fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
}

fn remove_dir_all_if_empty(path: &Path) {
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return;
            }
        }
        Err(_) => return,
    }
    remove_dir_all_with_retry(path);
}

extern "C" fn cleanup_process_temp_paths() {
    if let Ok(dirs) = SHARED_TEMP_DIRS.lock() {
        for dir in dirs.iter() {
            remove_dir_all_if_empty(dir);
        }
    }

    // 测试构建下 `db::get_home_app_dir` / `db::get_bootstrap_base_dir` 使用按进程号固定的目录，
    // 它们由 `OnceLock` 惰性创建、无法用用例级守卫回收，这里按同一命名规则兜底清理。
    let pid = std::process::id();
    for prefix in [
        "jx3-raid-manager-test-home-",
        "jx3-raid-manager-test-bootstrap-",
    ] {
        remove_dir_all_with_retry(&std::env::temp_dir().join(format!("{}{}", prefix, pid)));
    }
}

/// 登记一个「退出时若为空则删除」的共享临时目录。
pub(crate) fn register_shared_temp_dir(path: &Path) {
    ATEXIT_REGISTERED.call_once(|| unsafe {
        atexit(cleanup_process_temp_paths);
    });

    if let Ok(mut dirs) = SHARED_TEMP_DIRS.lock() {
        if !dirs.iter().any(|existing| existing == path) {
            dirs.push(path.to_path_buf());
        }
    }
}
