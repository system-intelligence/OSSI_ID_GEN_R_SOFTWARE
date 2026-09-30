use std::path::{Path, PathBuf};

// Name of the folder in the user's Documents that holds an installed app's data
pub const WORK_FOLDER_NAME: &str = "OSSI ID Generator";

// Where the app reads its templates (resources) and keeps its working files (work: data/, ID/, VP-SIGNATURE.png).
//
// - OSSI_BASE_DIR set: that one folder is used for both.
// - Development (cargo run / target\release): the project folder above the executable that holds
//   templates/front-id.svg is used for both, as before.
// - Installed: the templates are bundled next to the executable, and the working files go to
//   Documents\OSSI ID Generator, which is writable, easy to find for CorelDRAW, and survives updates and uninstalls.
pub fn locate(documents_dir: Option<PathBuf>) -> (PathBuf, PathBuf) {
    if let Ok(dir) = std::env::var("OSSI_BASE_DIR") {
        if !dir.trim().is_empty() {
            let dir = PathBuf::from(dir);
            return (dir.clone(), dir);
        }
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    locate_from(&exe_dir, documents_dir)
}

fn has_templates(dir: &Path) -> bool {
    dir.join("templates").join("front-id.svg").is_file()
}

fn locate_from(exe_dir: &Path, documents_dir: Option<PathBuf>) -> (PathBuf, PathBuf) {
    // Folders above the executable first: during development the build also copies the templates next to the
    // executable (target\debug\templates), but the project folder above it is the one to use
    if let Some(project) = exe_dir.ancestors().skip(1).find(|dir| has_templates(dir)) {
        return (project.to_path_buf(), project.to_path_buf());
    }
    if has_templates(exe_dir) {
        let work = documents_dir.map(|docs| docs.join(WORK_FOLDER_NAME)).unwrap_or_else(|| exe_dir.join("work"));
        return (exe_dir.to_path_buf(), work);
    }
    (exe_dir.to_path_buf(), exe_dir.to_path_buf())
}

pub struct Paths {
    pub data_dir: PathBuf,
    pub database: PathBuf,
    pub pin_config: PathBuf,
    pub front_template: PathBuf,
    pub back_template: PathBuf,
    pub id_output: PathBuf,
    pub authorized_signature: PathBuf,
}

// data/<file>, unless only the Electron app's src/<file> exists (OSSI_BASE_DIR pointed at the old project)
fn data_file(base: &Path, name: &str) -> PathBuf {
    let current = base.join("data").join(name);
    let legacy = base.join("src").join(name);
    if !current.exists() && legacy.is_file() { legacy } else { current }
}

impl Paths {
    pub fn new(resources: PathBuf, work: PathBuf) -> Self {
        Paths {
            data_dir: work.join("data"),
            database: data_file(&work, "id-generator.db"),
            pin_config: data_file(&work, "pin-config.json"),
            front_template: resources.join("templates").join("front-id.svg"),
            back_template: resources.join("templates").join("back-id.svg"),
            id_output: work.join("ID"),
            authorized_signature: work.join("VP-SIGNATURE.png"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ossi-paths-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add_templates(dir: &Path) {
        fs::create_dir_all(dir.join("templates")).unwrap();
        fs::write(dir.join("templates").join("front-id.svg"), "<svg/>").unwrap();
    }

    #[test]
    fn development_uses_the_project_folder_even_with_copied_templates() {
        let project = temp_dir("dev");
        add_templates(&project);
        let exe_dir = project.join("src-tauri").join("target").join("debug");
        add_templates(&exe_dir); // the build copies resources next to the dev executable
        let (resources, work) = locate_from(&exe_dir, Some(project.join("Documents")));
        assert_eq!(resources, project);
        assert_eq!(work, project);
        fs::remove_dir_all(&project).unwrap();
    }

    #[test]
    fn installed_app_keeps_its_data_in_documents() {
        let root = temp_dir("installed");
        let install = root.join("AppData").join("Local").join("ID Card Generator");
        add_templates(&install);
        let docs = root.join("Documents");
        let (resources, work) = locate_from(&install, Some(docs.clone()));
        assert_eq!(resources, install);
        assert_eq!(work, docs.join(WORK_FOLDER_NAME));

        let paths = Paths::new(resources, work);
        assert_eq!(paths.front_template, install.join("templates").join("front-id.svg"));
        assert_eq!(paths.database, docs.join(WORK_FOLDER_NAME).join("data").join("id-generator.db"));
        assert_eq!(paths.id_output, docs.join(WORK_FOLDER_NAME).join("ID"));
        assert_eq!(paths.authorized_signature, docs.join(WORK_FOLDER_NAME).join("VP-SIGNATURE.png"));
        fs::remove_dir_all(&root).unwrap();
    }
}
