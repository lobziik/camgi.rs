use std::fs;
use std::path::{Path, PathBuf};

use yaml_rust::{Yaml,YamlLoader};
use anyhow::{anyhow, Result, Context};

use crate::mustgather::MustGatherSubPath;

pub struct K8sManifest {
    source_path: PathBuf,
    raw: Option<String>,
    yaml: Yaml
}

// TODO Consider serde

impl K8sManifest {
    fn read_dir(path: &PathBuf) -> Result<Vec<K8sManifest>> {
        if !path.is_dir() {
            return Err(anyhow!("Path is not directory {}", path.as_path().display()));
        }
        let files =  fs::read_dir(&path)?;

        let yaml_documents: Vec<PathBuf> = files
            .into_iter()
            .filter(|f| {
                match f {
                    Ok(f) => f.path().is_file() && f.path().extension().unwrap() == "yaml",
                    Err(_) => false
                }
            })
            .map(|f| f.unwrap().path())
            .collect();

        let mut manifests = Vec::<K8sManifest>::new();

        for file_path in yaml_documents {
            let raw_content = fs::read_to_string(file_path.clone())?;
            let parsed_yaml = YamlLoader::load_from_str(&raw_content)?;

            if parsed_yaml.len() != 1 {
                return Err(
                    anyhow!(
                        "{} expected to contain only one yaml document",
                        file_path.display()
                    )
                )
            }

            manifests.push(
                K8sManifest {
                    source_path: file_path.clone(),
                    raw: Some(raw_content.to_owned()),
                    yaml: parsed_yaml[0].to_owned(),
                }
            )
        }

        Ok(manifests)
    }

    fn read_file(path: &PathBuf) -> Result<Vec<K8sManifest>> {
        if !path.is_file() {
            return Err(anyhow!("Path is not a file {}", path.as_path().display()));
        }

        let raw_content = fs::read_to_string(path.clone())?;
        let parsed_yaml = YamlLoader::load_from_str(&raw_content)?;
        if parsed_yaml.len() != 1 {
            return Err(
                anyhow!(
                    "{} expected to contain only one yaml document",
                    path.display()
                )
            )
        }

        let maybe_list_type_resource = parsed_yaml[0].to_owned();

        match &maybe_list_type_resource["items"] {
            Yaml::Array(items) => {
                let manifests: Vec<K8sManifest> = items.into_iter().map(
                    |i| {
                        K8sManifest {
                            source_path: path.clone(),
                            raw: None, // TODO try to use serde to get element in string format
                            yaml: i.clone()
                        }
                    }
                ).collect();
                Ok(manifests)
            },
            _ => Err(anyhow!("doesnt look like list type resource"))
        }
    }
}

pub enum K8sResourceScope {
    Namespaced,
    Cluster,
    ClusterSingleton,
}

pub trait MustGatherK8sResource {
    fn get_group() -> String;
    fn get_kind() -> String;
    fn get_kind_plural() -> String { format!("{}s", Self::get_kind()) }
    fn get_resource_scope() -> K8sResourceScope;

    fn get_resources_root(mg: impl MustGatherSubPath) -> PathBuf {
        let mut root = mg.get_path();
        root.push(Self::get_group());
        root.push(Self::get_kind_plural());
        root
    }

    fn get_all_resources(mg: impl MustGatherSubPath) -> Result<Vec<K8sManifest>> {
        let resources_root = Self::get_resources_root(mg);
        if resources_root.is_dir() {
            return K8sManifest::read_dir(&resources_root);
        }

        let mut resources_document = resources_root
            .parent()
            .context(format!("{} does not have parent directory", resources_root.display()))?
            .to_path_buf();
        resources_document.push(format!("{}.yaml", Self::get_kind_plural()));
        if resources_document.is_file() {
            return K8sManifest::read_file(&resources_document);
        }
        Err(anyhow!("can not found sutiable manifests in {}", resources_root.display()))
    }
}

#[cfg(test)]
mod tests {
    use crate::mustgather::MustGatherRoot;
    use super::*;

    struct Machine {}
    impl MustGatherK8sResource for Machine {
        fn get_group() -> String { String::from("machine.openshift.io") }
        fn get_kind() -> String { String::from("machine") }
        fn get_resource_scope() -> K8sResourceScope { K8sResourceScope::Namespaced }
    }

    struct Deployment {}
    impl MustGatherK8sResource for Deployment {
        fn get_group() -> String { String::from("apps") }
        fn get_kind() -> String { String::from("deployment") }
        fn get_resource_scope() -> K8sResourceScope { K8sResourceScope::Namespaced }
    }

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

    #[test]
    fn test_resources_in_multiple_files() {
        let root = MustGatherRoot::new(get_valid_mg_path().as_path()).unwrap();
        let ns = root.new_namespace("openshift-machine-api".to_string());

        let manifests = Machine::get_all_resources(ns).unwrap();
        assert_eq!(manifests.len(), 3)
    }

    #[test]
    fn test_resourcelist_in_single_document() {
        let root = MustGatherRoot::new(get_valid_mg_path().as_path()).unwrap();
        let ns = root.new_namespace("openshift-machine-api".to_string());

        let manifests = Deployment::get_all_resources(ns).unwrap();
        assert_eq!(manifests.len(), 5)
    }
}