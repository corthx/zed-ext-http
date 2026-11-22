//! Mirrors where httpyac looks for JetBrains env files: each ancestor folder and its `env/`.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_FILE: &str = "http-client.env.json";
const PRIVATE_FILE: &str = "http-client.private.env.json";
const SHARED: &str = "$shared";

#[derive(Debug)]
pub struct Environments {
    /// Folder httpyac must be able to see from its working directory.
    pub root: PathBuf,
    /// Folder holding the env files; selections are remembered per env folder.
    pub env_dir: PathBuf,
    envs: BTreeMap<String, Map<String, Value>>,
}

impl Environments {
    pub fn discover(http_file: &Path) -> Result<Option<Self>> {
        let Some(start) = http_file.parent() else {
            return Ok(None);
        };
        for dir in start.ancestors() {
            for candidate in [dir.to_path_buf(), dir.join("env")] {
                let files: Vec<_> = [PUBLIC_FILE, PRIVATE_FILE]
                    .iter()
                    .map(|f| candidate.join(f))
                    .filter(|p| p.is_file())
                    .collect();
                if files.is_empty() {
                    continue;
                }
                let mut envs: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
                for file in files {
                    for (name, vars) in read_object(&file)? {
                        if let Value::Object(vars) = vars {
                            envs.entry(name).or_default().extend(vars);
                        }
                    }
                }
                return Ok(Some(Self {
                    root: dir.to_path_buf(),
                    env_dir: candidate,
                    envs,
                }));
            }
        }
        Ok(None)
    }

    pub fn names(&self) -> Vec<String> {
        self.envs
            .keys()
            .filter(|n| !n.starts_with('$'))
            .cloned()
            .collect()
    }

    pub fn check_known(&self, name: &str) -> Result<()> {
        if !self.names().iter().any(|n| n == name) {
            bail!(
                "unknown environment \"{name}\" (available: {})",
                self.names().join(", ")
            );
        }
        Ok(())
    }

    /// `$shared` scalars the selected env does not define, as `key=value` (httpyac ignores `$shared`).
    pub fn shared_vars(&self, env: Option<&str>) -> Vec<String> {
        let Some(shared) = self.envs.get(SHARED) else {
            return Vec::new();
        };
        let own = env.and_then(|e| self.envs.get(e));
        shared
            .iter()
            .filter(|(key, _)| !own.is_some_and(|o| o.contains_key(*key)))
            .filter_map(|(key, value)| {
                let value = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => return None,
                };
                Some(format!("{key}={value}"))
            })
            .collect()
    }

    pub fn saved_selection(&self) -> Option<String> {
        let saved = load_selections()
            .get(&*self.env_dir.to_string_lossy())?
            .as_str()?
            .to_string();
        self.names().contains(&saved).then_some(saved)
    }

    pub fn save_selection(&self, env: Option<&str>) -> Result<()> {
        let mut selections = load_selections();
        let key = self.env_dir.to_string_lossy().to_string();
        match env {
            Some(env) => selections.insert(key, Value::String(env.into())),
            None => selections.remove(&key),
        };
        let path = selections_file()?;
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, serde_json::to_string_pretty(&selections)? + "\n")?;
        Ok(())
    }
}

fn read_object(path: &Path) -> Result<Map<String, Value>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    match serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))? {
        Value::Object(map) => Ok(map),
        _ => bail!("{}: expected a JSON object of environments", path.display()),
    }
}

fn selections_file() -> Result<PathBuf> {
    Ok(crate::paths::config_dir()?.join("selected-envs.json"))
}

fn load_selections() -> Map<String, Value> {
    selections_file()
        .ok()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_in_ancestor_and_merges_private() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            &root.join(PUBLIC_FILE),
            r#"{"$shared": {"ua": "runner", "host": "shared", "nested": {"a": 1}},
                "dev": {"host": "localhost"}, "prod": {"host": "example.com"}}"#,
        );
        write(&root.join(PRIVATE_FILE), r#"{"dev": {"token": "secret"}}"#);
        let http = root.join("api/v1/users.http");
        write(&http, "GET http://x");

        let envs = Environments::discover(&http).unwrap().unwrap();
        assert_eq!(envs.root, root);
        assert_eq!(envs.env_dir, root);
        assert_eq!(envs.names(), ["dev", "prod"]);
        assert_eq!(envs.envs["dev"]["token"], "secret");
        assert_eq!(envs.shared_vars(Some("dev")), ["ua=runner"]);
        assert_eq!(envs.shared_vars(None), ["host=shared", "ua=runner"]);
        assert!(envs.check_known("nope").is_err());
    }

    #[test]
    fn finds_env_folder() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("env").join(PUBLIC_FILE),
            r#"{"local": {}}"#,
        );
        let envs = Environments::discover(&tmp.path().join("a.http"))
            .unwrap()
            .unwrap();
        assert_eq!(envs.root, tmp.path());
        assert_eq!(envs.env_dir, tmp.path().join("env"));
    }

    #[test]
    fn none_without_env_files() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            Environments::discover(&tmp.path().join("a.http"))
                .unwrap()
                .is_none()
        );
    }
}
