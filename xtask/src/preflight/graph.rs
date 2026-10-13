//! Local Cargo package graph from `cargo metadata`, used for dependency impact.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPackage {
    pub name: String,
    /// Repository-relative directory, `/`-separated.
    pub dir: String,
    /// Names of local packages this one depends on (normal, dev and build).
    pub deps: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageGraph {
    pub packages: Vec<LocalPackage>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
}

#[derive(Deserialize)]
struct MetadataPackage {
    name: String,
    manifest_path: String,
    dependencies: Vec<MetadataDependency>,
}

#[derive(Deserialize)]
struct MetadataDependency {
    path: Option<String>,
}

impl PackageGraph {
    pub fn load(root: &Path) -> Result<Self> {
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
            .current_dir(root)
            .output()
            .context("could not run cargo metadata")?;
        if !output.status.success() {
            bail!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Self::from_metadata(root, &String::from_utf8_lossy(&output.stdout))
    }

    pub fn from_metadata(root: &Path, json: &str) -> Result<Self> {
        let metadata: Metadata =
            serde_json::from_str(json).context("unexpected cargo metadata output")?;
        let relative = |absolute: &Path| -> Result<String> {
            let dir = absolute.strip_prefix(root).with_context(|| {
                format!(
                    "local package is outside the repository: {}",
                    absolute.display()
                )
            })?;
            Ok(dir
                .to_str()
                .context("package path is not UTF-8")?
                .replace('\\', "/"))
        };
        let mut by_dir = BTreeMap::new();
        for package in &metadata.packages {
            let dir = Path::new(&package.manifest_path)
                .parent()
                .context("manifest has no parent directory")?;
            by_dir.insert(relative(dir)?, package.name.clone());
        }
        let mut packages = Vec::new();
        for package in &metadata.packages {
            let dir = relative(
                Path::new(&package.manifest_path)
                    .parent()
                    .context("manifest has no parent directory")?,
            )?;
            let mut deps = BTreeSet::new();
            for dependency in &package.dependencies {
                if let Some(path) = &dependency.path {
                    let dep_dir = relative(Path::new(path))?;
                    let name = by_dir.get(&dep_dir).with_context(|| {
                        format!(
                            "{} depends on {dep_dir}, which is not a workspace member",
                            package.name
                        )
                    })?;
                    deps.insert(name.clone());
                }
            }
            packages.push(LocalPackage {
                name: package.name.clone(),
                dir,
                deps,
            });
        }
        packages.sort_by(|a, b| b.dir.len().cmp(&a.dir.len()).then(a.dir.cmp(&b.dir)));
        Ok(Self { packages })
    }

    /// The package whose directory contains `path` (deepest directory wins).
    pub fn owner(&self, path: &str) -> Option<&LocalPackage> {
        self.packages
            .iter()
            .find(|package| super::select::within(path, &package.dir))
    }

    /// `changed` plus every package that depends on one of them, transitively.
    pub fn dependents_closure(&self, changed: &BTreeSet<String>) -> BTreeSet<String> {
        let mut affected = changed.clone();
        loop {
            let before = affected.len();
            for package in &self.packages {
                if package.deps.iter().any(|dep| affected.contains(dep)) {
                    affected.insert(package.name.clone());
                }
            }
            if affected.len() == before {
                return affected;
            }
        }
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// The current Cantos graph, written out so selection tests do not run Cargo.
    pub fn cantos() -> PackageGraph {
        let package = |name: &str, dir: &str, deps: &[&str]| LocalPackage {
            name: name.into(),
            dir: dir.into(),
            deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
        };
        PackageGraph {
            packages: vec![
                package("cantos-forge", "apps/cantos-forge", &[]),
                package(
                    "cantos-studio",
                    "apps/cantos-studio/web",
                    &["cantos-contracts", "cantos-ui"],
                ),
                package("cantos-contracts", "packages/contracts/api", &[]),
                package("cantos-api", "services/cantos-api", &["cantos-contracts"]),
                package(
                    "cantos-theatre",
                    "apps/cantos-theatre/web",
                    &["cantos-contracts", "cantos-ui"],
                ),
                package("cantos-ui", "packages/ui-system/leptos", &[]),
                package("xtask", "xtask", &[]),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_path_dependencies_from_metadata() {
        let json = r#"{"packages":[
            {"name":"cantos-api","manifest_path":"/r/services/cantos-api/Cargo.toml",
             "dependencies":[{"name":"cantos-contracts","path":"/r/packages/contracts/api"},
                             {"name":"serde"}]},
            {"name":"cantos-contracts","manifest_path":"/r/packages/contracts/api/Cargo.toml",
             "dependencies":[]}]}"#;
        let graph = PackageGraph::from_metadata(Path::new("/r"), json).unwrap();
        let api = graph.owner("services/cantos-api/src/lib.rs").unwrap();
        assert_eq!(api.name, "cantos-api");
        assert_eq!(api.deps.iter().collect::<Vec<_>>(), ["cantos-contracts"]);
        assert!(graph.owner("services/kanidm/README.md").is_none());
    }

    #[test]
    fn rejects_dependencies_outside_the_repository() {
        let json = r#"{"packages":[{"name":"a","manifest_path":"/r/a/Cargo.toml",
            "dependencies":[{"name":"b","path":"/elsewhere/b"}]}]}"#;
        assert!(PackageGraph::from_metadata(Path::new("/r"), json).is_err());
    }

    #[test]
    fn closure_includes_transitive_dependents() {
        let graph = testing::cantos();
        let affected = graph.dependents_closure(&["cantos-contracts".to_owned()].into());
        assert_eq!(
            affected.into_iter().collect::<Vec<_>>(),
            [
                "cantos-api",
                "cantos-contracts",
                "cantos-studio",
                "cantos-theatre"
            ]
        );
    }

    /// The documented direction: apps depend only on packages, services only on packages,
    /// packages on nothing outside `packages/`, and nothing depends on `xtask`.
    fn inward_violations(graph: &PackageGraph) -> Vec<String> {
        let area = |dir: &str| dir.split('/').next().unwrap_or_default().to_owned();
        let product = |dir: &str| dir.split('/').take(2).collect::<Vec<_>>().join("/");
        let by_name: BTreeMap<_, _> = graph
            .packages
            .iter()
            .map(|package| (package.name.as_str(), package.dir.as_str()))
            .collect();
        let mut violations = Vec::new();
        for package in &graph.packages {
            for dep in &package.deps {
                let Some(dep_dir) = by_name.get(dep.as_str()) else {
                    violations.push(format!("{} -> {dep}: unknown package", package.name));
                    continue;
                };
                let allowed = match (area(&package.dir).as_str(), area(dep_dir).as_str()) {
                    (_, "xtask") => false,
                    ("apps", "apps") => product(&package.dir) == product(dep_dir),
                    ("apps" | "services" | "packages" | "xtask", "packages") => true,
                    _ => false,
                };
                if !allowed {
                    violations.push(format!(
                        "{} ({}) -> {dep} ({dep_dir})",
                        package.name, package.dir
                    ));
                }
            }
        }
        violations
    }

    #[test]
    fn workspace_dependencies_point_inward() {
        let root = crate::workspace::Workspace::discover().unwrap();
        let real = PackageGraph::load(root.root()).unwrap();
        assert_eq!(inward_violations(&real), Vec::<String>::new());
        assert_eq!(inward_violations(&testing::cantos()), Vec::<String>::new());
    }

    #[test]
    fn the_inward_rule_rejects_each_forbidden_edge() {
        let mut graph = testing::cantos();
        let edges = [
            ("cantos-forge", "cantos-api"),
            ("cantos-forge", "cantos-studio"),
            ("cantos-contracts", "cantos-ui"),
            ("cantos-contracts", "cantos-api"),
            ("cantos-api", "cantos-studio"),
            ("cantos-ui", "xtask"),
        ];
        for (from, to) in edges {
            let package = graph
                .packages
                .iter_mut()
                .find(|package| package.name == from)
                .unwrap();
            package.deps.insert(to.to_owned());
        }
        assert_eq!(
            inward_violations(&graph),
            [
                "cantos-forge (apps/cantos-forge) -> cantos-api (services/cantos-api)",
                "cantos-forge (apps/cantos-forge) -> cantos-studio (apps/cantos-studio/web)",
                "cantos-contracts (packages/contracts/api) -> cantos-api (services/cantos-api)",
                "cantos-api (services/cantos-api) -> cantos-studio (apps/cantos-studio/web)",
                "cantos-ui (packages/ui-system/leptos) -> xtask (xtask)",
            ]
        );
    }

    #[test]
    fn the_real_workspace_matches_the_test_graph() {
        let root = crate::workspace::Workspace::discover().unwrap();
        let mut real = PackageGraph::load(root.root()).unwrap().packages;
        let mut expected = testing::cantos().packages;
        real.sort_by(|a, b| a.name.cmp(&b.name));
        expected.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(real, expected);
    }
}
