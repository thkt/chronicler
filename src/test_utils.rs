use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chronicler-test-{}-{}-{}",
            prefix,
            std::process::id(),
            id
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

pub fn set_mtime_past(path: &Path, secs_ago: u64) {
    use std::fs::{File, FileTimes};
    use std::time::{Duration, SystemTime};

    let past = SystemTime::now() - Duration::from_secs(secs_ago);
    let file = File::options().write(true).open(path).unwrap();
    file.set_times(FileTimes::new().set_modified(past)).unwrap();
}

pub fn set_mtime(path: &Path, time: std::time::SystemTime) {
    use std::fs::{File, FileTimes};

    let file = File::options().write(true).open(path).unwrap();
    file.set_times(FileTimes::new().set_modified(time)).unwrap();
}
