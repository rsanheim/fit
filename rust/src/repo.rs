use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Find all git repositories in the current directory (depth 1).
/// Returns a sorted list of paths to directories containing a .git folder.
pub fn find_git_repos() -> Result<Vec<PathBuf>> {
    let cwd = std::env::current_dir()?;
    let mut repos = Vec::new();

    for entry in fs::read_dir(&cwd)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let git_dir = path.join(".git");
            if git_dir.exists() {
                repos.push(path);
            }
        }
    }

    repos.sort();
    Ok(repos)
}

/// Extract just the repository name from a path
pub fn repo_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_name() {
        let path = PathBuf::from("/home/user/src/my-repo");
        assert_eq!(repo_name(&path), "my-repo");
    }

    #[test]
    fn test_repo_name_root() {
        let path = PathBuf::from("/");
        assert_eq!(repo_name(&path), "unknown");
    }
}
