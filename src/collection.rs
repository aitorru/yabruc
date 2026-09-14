//! Loads a Bruno collection from disk.
//!
//! The collection root is the closest folder with an `opencollection.yml` (YAML collections) or a
//! `bruno.json` (bru collections). Files outside of a collection can also be run, in that case
//! both formats are accepted and there are no defaults nor environments.
//!
//! Like `bru run`, folders are sorted by name and then by their `seq`, and requests by their `seq`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    model::{Defaults, Environment, Request},
    parser::{self, bru, bru2struct, yml2struct},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Bru,
    Yml,
}

impl Format {
    fn collection_file(self) -> &'static str {
        match self {
            Format::Bru => "collection.bru",
            Format::Yml => "opencollection.yml",
        }
    }

    fn folder_file(self) -> &'static str {
        match self {
            Format::Bru => "folder.bru",
            Format::Yml => "folder.yml",
        }
    }

    fn of(path: &Path) -> Option<Format> {
        match path.extension()?.to_str()? {
            "bru" => Some(Format::Bru),
            "yml" | "yaml" => Some(Format::Yml),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Collection {
    pub root: PathBuf,
    /// `None` when the files are not inside a collection.
    pub format: Option<Format>,
    pub name: String,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used to build the requests in step 3 of #5")
    )]
    pub defaults: Defaults,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used to build the requests in step 3 of #5")
    )]
    pub environments: Vec<Environment>,
    /// Problems that do not stop the run, like an environment that can not be parsed.
    pub warnings: Vec<String>,
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Folder(Folder),
    Request(RequestFile),
}

#[derive(Debug)]
pub struct Folder {
    /// Name of the directory.
    pub name: String,
    pub defaults: Defaults,
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct RequestFile {
    pub path: PathBuf,
    pub request: Result<Request, String>,
}

impl Collection {
    /// Returns the requests inside `path`, which can be the collection, a folder or a file, in the
    /// order they must run.
    pub fn requests_in(&self, path: &Path) -> Vec<&RequestFile> {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut requests = vec![];
        collect_requests(&self.items, &mut requests);
        requests.retain(|request| request.path.starts_with(&path));
        requests
    }
}

fn collect_requests<'a>(items: &'a [Item], requests: &mut Vec<&'a RequestFile>) {
    for item in items {
        match item {
            Item::Folder(folder) => collect_requests(&folder.items, requests),
            Item::Request(request) => requests.push(request),
        }
    }
}

pub fn load(path: &Path) -> Result<Collection, String> {
    let path = fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let start = if path.is_dir() {
        path.as_path()
    } else {
        path.parent().unwrap_or(&path)
    };

    let found = start.ancestors().find_map(|dir| {
        if dir.join("opencollection.yml").is_file() {
            Some((dir, Format::Yml))
        } else if dir.join("bruno.json").is_file() {
            Some((dir, Format::Bru))
        } else {
            None
        }
    });

    match found {
        Some((root, format)) => load_collection(root, format),
        None if path.is_dir() => Ok(loose_collection(&path, Loader::loose(&path).items(&path))),
        None => {
            let request = RequestFile {
                request: parse_request(&path),
                path: path.clone(),
            };
            Ok(loose_collection(start, vec![Item::Request(request)]))
        }
    }
}

fn loose_collection(root: &Path, items: Vec<Item>) -> Collection {
    Collection {
        root: root.to_path_buf(),
        format: None,
        name: root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        defaults: Defaults::default(),
        environments: vec![],
        warnings: vec![],
        items,
    }
}

fn load_collection(root: &Path, format: Format) -> Result<Collection, String> {
    let read = |name: &str| {
        let path = root.join(name);
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let (name, ignore, defaults) = match format {
        Format::Yml => {
            let config = yml2struct::parse_collection(&read("opencollection.yml")?)
                .map_err(|error| format!("opencollection.yml: {error}"))?;
            (config.name, config.ignore, config.defaults)
        }
        Format::Bru => {
            let config: serde_json::Value = serde_json::from_str(&read("bruno.json")?)
                .map_err(|error| format!("bruno.json: {error}"))?;
            let defaults = if root.join("collection.bru").is_file() {
                let document = bru::parse(&read("collection.bru")?)
                    .map_err(|error| format!("collection.bru: {error}"))?;
                bru2struct::document_to_defaults(&document)
            } else {
                Defaults::default()
            };
            let ignore = config["ignore"]
                .as_array()
                .map(|ignore| {
                    ignore
                        .iter()
                        .filter_map(|pattern| Some(pattern.as_str()?.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            (
                config["name"].as_str().unwrap_or_default().to_string(),
                ignore,
                defaults,
            )
        }
    };

    let loader = Loader {
        root: root.to_path_buf(),
        format: Some(format),
        ignore,
    };
    let (environments, warnings) = loader.environments();
    Ok(Collection {
        root: root.to_path_buf(),
        format: Some(format),
        name,
        defaults,
        environments,
        warnings,
        items: loader.items(root),
    })
}

struct Loader {
    root: PathBuf,
    format: Option<Format>,
    ignore: Vec<String>,
}

impl Loader {
    fn loose(root: &Path) -> Loader {
        Loader {
            root: root.to_path_buf(),
            format: None,
            ignore: vec![],
        }
    }

    fn is_ignored(&self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        let relative = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>();
        if relative
            .iter()
            .any(|segment| segment == "node_modules" || segment == ".git")
        {
            return true;
        }
        if relative.len() == 1 && relative[0] == "environments" {
            return true;
        }
        let relative = relative.join("/");
        self.ignore.iter().any(|pattern| {
            let pattern = pattern.replace('\\', "/");
            !pattern.is_empty()
                && (relative == pattern || relative.starts_with(&format!("{pattern}/")))
        })
    }

    /// Whether the file is a request, and not a collection or folder file.
    fn is_request_file(&self, path: &Path) -> bool {
        let Some(format) = Format::of(path) else {
            return false;
        };
        if self.format.is_some_and(|expected| expected != format) {
            return false;
        }
        let name = path.file_name().unwrap_or_default();
        [Format::Bru, Format::Yml]
            .iter()
            .all(|format| name != format.collection_file() && name != format.folder_file())
    }

    fn items(&self, dir: &Path) -> Vec<Item> {
        let mut entries = match fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect::<Vec<_>>(),
            Err(error) => {
                return vec![Item::Request(RequestFile {
                    path: dir.to_path_buf(),
                    request: Err(format!("could not read the folder: {error}")),
                })];
            }
        };
        entries.sort();

        let mut folders = vec![];
        let mut requests = vec![];
        for path in entries {
            if self.is_ignored(&path) {
                continue;
            }
            // Symbolic links to folders are not followed, like in Bruno
            let is_dir = fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir());
            if is_dir {
                let (defaults, error) = self.folder_defaults(&path);
                let mut items = self.items(&path);
                if let Some(error) = error {
                    items.insert(0, Item::Request(error));
                }
                folders.push(Folder {
                    name: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    defaults,
                    items,
                });
            } else if self.is_request_file(&path) {
                requests.push(RequestFile {
                    request: parse_request(&path),
                    path,
                });
            }
        }

        // Files that could not be parsed run last
        requests.sort_by(|a, b| seq(a).total_cmp(&seq(b)));
        sort_folders(folders)
            .into_iter()
            .map(Item::Folder)
            .chain(requests.into_iter().map(Item::Request))
            .collect()
    }

    fn folder_defaults(&self, dir: &Path) -> (Defaults, Option<RequestFile>) {
        let formats = match self.format {
            Some(format) => vec![format],
            None => vec![Format::Bru, Format::Yml],
        };
        let Some((path, format)) = formats
            .into_iter()
            .map(|format| (dir.join(format.folder_file()), format))
            .find(|(path, _)| path.is_file())
        else {
            return (Defaults::default(), None);
        };

        let defaults = fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|source| match format {
                Format::Bru => bru::parse(&source)
                    .map(|document| bru2struct::document_to_defaults(&document))
                    .map_err(|error| error.to_string()),
                Format::Yml => yml2struct::parse_folder(&source),
            });
        match defaults {
            Ok(defaults) => (defaults, None),
            Err(error) => (
                Defaults::default(),
                Some(RequestFile {
                    path,
                    request: Err(error),
                }),
            ),
        }
    }

    fn environments(&self) -> (Vec<Environment>, Vec<String>) {
        let mut environments = vec![];
        let mut warnings = vec![];
        let Ok(entries) = fs::read_dir(self.root.join("environments")) else {
            return (environments, warnings);
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .collect();
        paths.sort();

        for path in paths {
            let Some(format) = Format::of(&path).filter(|format| Some(*format) == self.format)
            else {
                continue;
            };
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let environment = fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|source| match format {
                    Format::Bru => bru::parse(&source)
                        .map(|document| bru2struct::document_to_environment(&name, &document))
                        .map_err(|error| error.to_string()),
                    Format::Yml => yml2struct::parse_environment(&name, &source),
                });
            match environment {
                Ok(environment) => environments.push(environment),
                Err(error) => warnings.push(format!(
                    "Could not parse the environment {}: {error}",
                    path.strip_prefix(&self.root).unwrap_or(&path).display()
                )),
            }
        }
        (environments, warnings)
    }
}

fn parse_request(path: &Path) -> Result<Request, String> {
    fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|source| parser::parse_request(path, &source))
}

fn seq(request: &RequestFile) -> f64 {
    request
        .request
        .as_ref()
        .map_or(f64::INFINITY, |request| request.seq)
}

/// Sorts folders by name, and then moves the ones with a `seq` to that position.
/// Same as `sortByNameThenSequence` in `@usebruno/common`.
fn sort_folders(mut folders: Vec<Folder>) -> Vec<Folder> {
    fn valid_seq(folder: &Folder) -> Option<usize> {
        let seq = folder.defaults.seq?;
        (seq.fract() == 0.0 && seq >= 1.0).then_some(seq as usize)
    }

    folders.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    let (mut with_seq, without_seq): (Vec<Folder>, Vec<Folder>) = folders
        .into_iter()
        .partition(|folder| valid_seq(folder).is_some());
    with_seq.sort_by_key(valid_seq);

    let mut groups: Vec<Vec<Folder>> = without_seq.into_iter().map(|folder| vec![folder]).collect();
    for folder in with_seq {
        let seq = valid_seq(&folder);
        let position = seq.unwrap_or(1) - 1;
        match groups.get_mut(position) {
            Some(group) if valid_seq(&group[0]) == seq => group.push(folder),
            _ => groups.insert(position.min(groups.len()), vec![folder]),
        }
    }
    groups.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Auth;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test/fixtures/collections")
            .join(name)
    }

    fn relative_paths(collection: &Collection, requests: &[&RequestFile]) -> Vec<String> {
        requests
            .iter()
            .map(|request| {
                request
                    .path
                    .strip_prefix(&collection.root)
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn loads_bru_collections() {
        let root = fixture("bru");
        let collection = load(&root).unwrap();
        assert_eq!(collection.format, Some(Format::Bru));
        assert_eq!(collection.name, "Bru fixture");
        assert_eq!(collection.defaults.headers[0].name, "x-collection");
        assert_eq!(
            collection.defaults.auth,
            Auth::Bearer {
                token: "abc".into()
            }
        );

        let requests = collection.requests_in(&root);
        assert_eq!(
            relative_paths(&collection, &requests),
            [
                "c-folder/two.bru",
                "b-folder/one.bru",
                "a-folder/deep/nested.bru",
                "a-folder/three.bru",
                "root-first.bru",
                "root-second.bru",
                "broken.bru",
            ]
        );
        assert!(requests[..6].iter().all(|request| request.request.is_ok()));
        assert_eq!(
            requests[6].request.as_ref().unwrap_err(),
            "line 1: unclosed block `meta`, expected `}`"
        );

        assert_eq!(collection.environments.len(), 1);
        assert_eq!(collection.environments[0].name, "local");
        assert_eq!(collection.warnings.len(), 1);
        assert!(collection.warnings[0].contains("broken.bru"));

        let Item::Folder(folder) = &collection.items[1] else {
            panic!("expected a folder");
        };
        assert_eq!(folder.defaults.name.as_deref(), Some("Second"));
    }

    #[test]
    fn runs_folders_and_files_inside_a_collection() {
        let root = fixture("bru");
        let collection = load(&root.join("a-folder")).unwrap();
        assert_eq!(collection.root, fs::canonicalize(&root).unwrap());
        assert_eq!(
            relative_paths(&collection, &collection.requests_in(&root.join("a-folder"))),
            ["a-folder/deep/nested.bru", "a-folder/three.bru"]
        );

        let file = root.join("root-second.bru");
        let collection = load(&file).unwrap();
        assert_eq!(
            relative_paths(&collection, &collection.requests_in(&file)),
            ["root-second.bru"]
        );
    }

    #[test]
    fn loads_yml_collections() {
        let root = fixture("yml");
        let collection = load(&root).unwrap();
        assert_eq!(collection.format, Some(Format::Yml));
        assert_eq!(collection.name, "Yml fixture");
        assert_eq!(collection.defaults.headers[0].name, "x-collection");
        assert_eq!(
            relative_paths(&collection, &collection.requests_in(&root)),
            ["users/create-user.yml", "get-users.yml", "list-teams.yml"]
        );
        assert_eq!(collection.environments[0].name, "Development");
        let Item::Folder(folder) = &collection.items[0] else {
            panic!("expected a folder");
        };
        assert_eq!(folder.defaults.auth, Auth::Inherit);
    }

    #[test]
    fn loads_files_outside_of_a_collection() {
        let root = fixture("loose");
        let collection = load(&root).unwrap();
        assert_eq!(collection.format, None);
        assert_eq!(
            relative_paths(&collection, &collection.requests_in(&root)),
            ["a.bru", "b.yml"]
        );

        let file = root.join("b.yml");
        let collection = load(&file).unwrap();
        assert_eq!(collection.requests_in(&file).len(), 1);
    }

    fn folder(name: &str, seq: Option<f64>) -> Folder {
        Folder {
            name: name.into(),
            defaults: Defaults {
                seq,
                ..Defaults::default()
            },
            items: vec![],
        }
    }

    #[test]
    fn sorts_folders_like_bruno() {
        let names = |folders: Vec<Folder>| {
            sort_folders(folders)
                .into_iter()
                .map(|folder| folder.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(vec![
                folder("a", Some(3.0)),
                folder("b", Some(2.0)),
                folder("c", None),
            ]),
            ["c", "b", "a"]
        );
        // Same seq are grouped, invalid seq are sorted by name
        assert_eq!(
            names(vec![
                folder("d", Some(1.0)),
                folder("B", Some(1.0)),
                folder("a", Some(0.5)),
                folder("c", Some(9.0)),
            ]),
            ["B", "d", "a", "c"]
        );
    }
}
