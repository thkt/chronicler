use std::env;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "chronicler-test-{}-{}-{}",
            prefix,
            process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

pub fn set_mtime_past(path: &Path, secs_ago: u64) {
    use std::fs::{File, FileTimes};
    use std::time::Duration;

    let past = SystemTime::now() - Duration::from_secs(secs_ago);
    let file = File::options().write(true).open(path).unwrap();
    file.set_times(FileTimes::new().set_modified(past)).unwrap();
}

pub fn set_mtime(path: &Path, time: SystemTime) {
    use std::fs::{File, FileTimes};

    let file = File::options().write(true).open(path).unwrap();
    file.set_times(FileTimes::new().set_modified(time)).unwrap();
}
