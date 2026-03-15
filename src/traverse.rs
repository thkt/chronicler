use std::path::Path;

const MAX_DEPTH: usize = 20;

pub fn find_project_root(start: &Path) -> Option<&Path> {
    let mut current = start;
    for _ in 0..MAX_DEPTH {
        if current.join(".git").exists() {
            return Some(current);
        }
        current = current.parent()?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    #[test]
    fn finds_root_at_start() {
        let tmp = TempDir::new("traverse-start");
        fs::create_dir_all(tmp.join(".git")).unwrap();

        let result = find_project_root(&tmp);
        assert_eq!(result, Some(tmp.as_ref()));
    }

    #[test]
    fn walks_up_to_find_root() {
        let tmp = TempDir::new("traverse-parent");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        let subdir = tmp.join("src/components");
        fs::create_dir_all(&subdir).unwrap();

        let result = find_project_root(&subdir);
        assert_eq!(result, Some(tmp.as_ref()));
    }

    #[test]
    fn returns_none_when_no_git() {
        let tmp = TempDir::new("traverse-none");

        let result = find_project_root(&tmp);
        assert!(result.is_none());
    }

    #[test]
    fn stops_at_nearest_git_boundary() {
        let tmp = TempDir::new("traverse-nested");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        let inner = tmp.join("packages/app");
        fs::create_dir_all(inner.join(".git")).unwrap();
        let src = inner.join("src");
        fs::create_dir_all(&src).unwrap();

        let result = find_project_root(&src);
        assert_eq!(result, Some(inner.as_path()));
    }
}
