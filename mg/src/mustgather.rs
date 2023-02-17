use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Clone)]
pub struct MustGatherRoot {
    path: PathBuf
}

impl MustGatherRoot {
    pub fn new(root: &Path) -> Result<MustGatherRoot> {
        let mg_path = Self::find_mustgather_root(root)?;

        Ok(MustGatherRoot {
            path: mg_path
        })
    }

    pub fn new_namespace(&self, ns: String) -> Namespace {
        Namespace {
            root: self.clone(),
            namespace: Some(ns)
        }
    }

    pub fn new_cluster_scoped(&self) -> Namespace {
        Namespace {
            root: self.clone(),
            namespace: None
        }
    }

    /// Find the root of a must-gather directory structure given a path.
    ///
    /// Finding the root of the must-gather is accomplished through the following steps:
    /// 1. look for a `version` file in the current path, if it exists return current path.
    /// 2. look for the directories `namespaces` and `cluster-scoped-resources` in the current path,
    ///    if they exist, return the current path.
    /// 3. if there is a single subdirectory in the path, recursively run this function on it and
    ///    return the result.
    /// 4. return an error
    fn find_mustgather_root(path: &Path) -> Result<PathBuf> {
        let version = path.join("version");
        let ns_dir = path.join("namespaces");
        let csr_dir = path.join("cluster-scoped-resources");

        if version.is_file() || (ns_dir.is_dir() && csr_dir.is_dir()) {
            return Ok(path.to_path_buf());
        }

        let sub_directories: Vec<PathBuf> = fs::read_dir(&path)?
            .into_iter()
            .filter(|r| {
                match r {
                    Ok(d) => d.path().is_dir(),
                    Err(_) => false
                }
            })
            .map(|dr| dr.unwrap().path())
            .collect();

        if sub_directories.len() == 1 {
            Self::find_mustgather_root(sub_directories[0].as_path())
        } else {
            Err(anyhow::anyhow!("Cannot determine root of must-gather"))
        }
    }
}

pub trait MustGatherSubPath {
    fn get_path(&self) -> PathBuf;
}

const CLUSTER_SCOPE_RESOURCES_PATH: &str = "cluster-scoped-resources";
const NAMESPACES_PREFIX: &str = "namespaces";

pub struct Namespace {
    root: MustGatherRoot,
    namespace: Option<String>
}

impl MustGatherSubPath for Namespace {
    fn get_path(&self) -> PathBuf {
        let mut path = self.root.path.clone();
        match self.namespace.clone() {
            Some(ns) => {
                path.push(NAMESPACES_PREFIX);
                path.push(ns)
            },
            None => path.push(CLUSTER_SCOPE_RESOURCES_PATH)
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_testdata_path() -> PathBuf {
        let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("..");
        p.push("testdata");
        p
    }

    fn get_valid_mg_path() -> PathBuf {
        let mut p = get_testdata_path();
        p.push("must-gather-valid");
        p
    }

    fn get_invalid_mg_path() -> PathBuf {
        let mut p = get_testdata_path();
        p.push("must-gather-invalid");
        p
    }

    #[test]
    fn test_mg_root_invalid() {
        let root = MustGatherRoot::new(get_invalid_mg_path().as_path());
        assert_eq!(
            root.is_err(),
            true
        );
        assert_eq!(
            root.err().unwrap().to_string(),
            "Cannot determine root of must-gather"
        )
    }

    #[test]
    fn test_namespace_path_get() {
        let root = MustGatherRoot::new(get_valid_mg_path().as_path()).unwrap();

        let ns = root.new_namespace("openshift-machine-api".to_string());
        let generated_path = ns.get_path();

        assert_eq!(
            generated_path.to_str().unwrap().ends_with("/sample-openshift-release/namespaces/openshift-machine-api"),
            true
        )
    }

    #[test]
    fn test_cluster_scoped_path_get() {
        let root = MustGatherRoot::new(get_valid_mg_path().as_path()).unwrap();

        let ns = root.new_cluster_scoped();
        let generated_path = ns.get_path();
        assert_eq!(
            generated_path.to_str().unwrap().ends_with("/sample-openshift-release/cluster-scoped-resources"),
            true
        )
    }
}
