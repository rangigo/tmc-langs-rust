use crate::{error::UtilError, task_executor};
use md5::{Context, Digest};
use serde_yaml::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tmc_langs_framework::{command::TmcCommand, io::file_util, subprocess::Redirection};
use walkdir::WalkDir;

#[derive(Debug, PartialEq, Eq)]
pub enum SourceBackend {
    Git,
}

#[derive(Debug)]
pub struct RefreshExercise {
    name: String,
    relative_path: PathBuf,
    available_points: Vec<String>,
}

#[derive(Debug)]
pub struct Course {
    name: String,
    cache_path: PathBuf,
    clone_path: PathBuf,
    stub_path: PathBuf,
    stub_zip_path: PathBuf,
    solution_path: PathBuf,
    solution_zip_path: PathBuf,
    exercises: Vec<RefreshExercise>,
    source_backend: SourceBackend,
    source_url: String,
    git_branch: String,
}

#[derive(Debug)]
pub struct Options {
    no_directory_changes: bool,
    no_background_operations: bool,
}

#[derive(Debug)]
pub struct RefreshData {
    pub new_exercises: Vec<String>,
    pub removed_exercises: Vec<String>,
    pub review_points: HashMap<String, Vec<String>>,
    pub metadata: HashMap<String, Mapping>,
    pub checksum_stubs: HashMap<String, Digest>,
    pub course_options: Mapping,
    pub update_points: HashMap<String, UpdatePoints>,
}

#[derive(Debug)]
struct ExerciseOptions {
    review_points: HashMap<String, Vec<String>>,
    metadata_map: HashMap<String, Mapping>,
}

#[derive(Debug)]
pub struct UpdatePoints {
    added: Vec<String>,
    removed: Vec<String>,
}

pub fn refresh_course(
    course: Course,
    options: Options,
    git_repos_chmod: Option<String>,
    git_repos_chgrp: Option<String>,
    cache_root: PathBuf,
    rails_root: PathBuf,
) -> Result<RefreshData, UtilError> {
    let old_cache_path = &course.cache_path;

    // increment_cached_version not implemented

    if !options.no_directory_changes {
        file_util::remove_dir_all(&course.cache_path)?;
        file_util::create_dir_all(&course.cache_path)?;
    }

    if !options.no_directory_changes {
        update_or_clone_repository(&course, &old_cache_path)?;
        check_directory_names(&course.clone_path)?;
    }

    let course_options = update_course_options(&course.clone_path, &course.name)?;

    // add_records_for_new_exercises & delete_records_for_removed_exercises
    let (new_exercises, removed_exercises) =
        update_exercises(&course.clone_path, &course.exercises)?;
    let ExerciseOptions {
        review_points,
        metadata_map: metadata,
    } = update_exercise_options(&course.exercises, &course.clone_path, &course.name)?;

    // set_has_tests_flags not implemented here

    let update_points = if !options.no_background_operations {
        update_available_points(&course.exercises, &course.clone_path, &review_points)?
    } else {
        HashMap::new()
    };

    if !options.no_directory_changes {
        // make_solutions
        task_executor::prepare_solutions(&[course.clone_path.clone()], &course.solution_path)?;
        // make_stubs
        let exercise_dirs = task_executor::find_exercise_directories(&course.clone_path)?;
        task_executor::prepare_stubs(exercise_dirs, &course.clone_path, &course.stub_path)?;
    }

    let checksum_stubs = checksum_stubs(&course.exercises, &course.stub_path)?;

    if !options.no_directory_changes {
        // make_zips_of_stubs
        execute_zip(&course.exercises, &course.stub_path, &course.stub_zip_path)?;

        // make_zips_of_solutions
        execute_zip(
            &course.exercises,
            &course.solution_path,
            &course.solution_zip_path,
        )?;

        // set_permissions
        set_permissions(
            &course.cache_path,
            git_repos_chmod,
            git_repos_chgrp,
            &cache_root,
            rails_root,
        )?;
    }

    // invalidate_unlocks not implemented
    // kafka_publish_exercises not implemented

    Ok(RefreshData {
        new_exercises,
        removed_exercises,
        review_points,
        metadata,
        checksum_stubs,
        course_options,
        update_points,
    })
}

fn update_or_clone_repository(course: &Course, old_cache_path: &Path) -> Result<(), UtilError> {
    if course.source_backend != SourceBackend::Git {
        log::error!("Source types other than git not yet implemented");
        return Err(UtilError::UnsupportedSourceBackend);
    }
    if old_cache_path.join("clone").join(".git").exists() {
        // Try a fast path: copy old clone and git fetch new stuff
        let copy_and_update_repository = || -> Result<(), UtilError> {
            let old_clone_path = old_cache_path.join("clone");
            file_util::copy(old_clone_path, &course.clone_path)?;

            let run_git = |args: &[&str]| {
                TmcCommand::new("git".to_string())
                    .with(|e| {
                        e.cwd(&course.clone_path)
                            .args(args)
                            .stdout(Redirection::Pipe)
                            .stderr(Redirection::Pipe)
                    })
                    .output_checked()
            };

            run_git(&["remote", "set-url", "origin", &course.source_url])?;
            run_git(&["fetch", "origin"])?;
            run_git(&["checkout", &format!("origin/{}", &course.git_branch)])?;
            run_git(&["clean", "-df"])?;
            run_git(&["checkout", "."])?;
            Ok(())
        };
        if let Err(error) = copy_and_update_repository() {
            log::warn!("failed to update repository: {}", error);

            file_util::remove_dir_all(&course.clone_path)?;
            // clone_repository
            TmcCommand::new("git".to_string())
                .with(|e| {
                    e.args(&["clone", "-q", "-b"])
                        .arg(&course.git_branch)
                        .arg(&course.source_url)
                        .arg(&course.clone_path)
                        .stdout(Redirection::Pipe)
                        .stderr(Redirection::Pipe)
                })
                .output_checked()?;
        }
    } else {
        // clone_repository
        TmcCommand::new("git".to_string())
            .with(|e| {
                e.args(&["clone", "-q", "-b"])
                    .arg(&course.git_branch)
                    .arg(&course.source_url)
                    .arg(&course.clone_path)
                    .stdout(Redirection::Pipe)
                    .stderr(Redirection::Pipe)
            })
            .output_checked()?;
    }
    Ok(())
}

fn check_directory_names(path: &Path) -> Result<(), UtilError> {
    // exercise directories in canonicalized form
    let exercise_dirs = {
        let mut canon_dirs = vec![];
        for path in task_executor::find_exercise_directories(path)? {
            let canon = path
                .canonicalize()
                .map_err(|e| UtilError::Canonicalize(path, e))?;
            canon_dirs.push(canon);
        }
        canon_dirs
    };
    for entry in WalkDir::new(path).min_depth(1) {
        let entry = entry?;
        let canon_path = entry
            .path()
            .canonicalize()
            .map_err(|e| UtilError::Canonicalize(entry.path().to_path_buf(), e))?;
        let relpath = entry.path().strip_prefix(path).unwrap();
        let rel_contains_dash = relpath.to_string_lossy().contains('-');
        if canon_path.is_dir()
            && exercise_dirs
                .iter()
                .any(|exdir| exdir.starts_with(&canon_path) && rel_contains_dash)
        {
            return Err(UtilError::InvalidDirectory(canon_path));
        }
    }
    Ok(())
}

fn update_course_options(
    course_clone_path: &Path,
    course_name: &str,
) -> Result<Mapping, UtilError> {
    let options_file = course_clone_path.join("course_options.yml");
    let opts = if options_file.exists() {
        let file = file_util::open_file(options_file)?;
        let mut course_options: Mapping = serde_yaml::from_reader(file).unwrap();
        merge_course_specific_options(course_name, &mut course_options);
        course_options
    } else {
        Mapping::new()
    };
    Ok(opts)
}

fn merge_course_specific_options(course_name: &str, opts: &mut Mapping) {
    // try to remove the "courses" map
    if let Some(serde_yaml::Value::Mapping(mut courses)) =
        opts.remove(&serde_yaml::Value::String("courses".to_string()))
    {
        // try to remove the map corresponding to the current course from the "courses" map
        if let Some(serde_yaml::Value::Mapping(mapping)) =
            courses.remove(&serde_yaml::Value::String(course_name.to_string()))
        {
            // if found, merge the inner course map with the base map
            for (key, value) in mapping {
                opts.insert(key, value);
            }
        }
    }
}

fn update_exercises(
    course_clone_path: &Path,
    course_exercises: &[RefreshExercise],
) -> Result<(Vec<String>, Vec<String>), UtilError> {
    let exercise_dirs = task_executor::find_exercise_directories(course_clone_path)?;
    let exercise_names = exercise_dirs
        .into_iter()
        .map(|ed| {
            ed.strip_prefix(course_clone_path)
                .unwrap_or(&ed)
                .to_string_lossy()
                .replace("/", "-")
        })
        .collect::<Vec<_>>();

    let mut new_exercises = vec![];
    for exercise_name in &exercise_names {
        if !course_exercises.iter().any(|e| &e.name == exercise_name) {
            log::info!("Added new exercise {}", exercise_name);
            new_exercises.push(exercise_name.clone());
        }
    }

    let mut removed_exercises = vec![];
    for exercise in course_exercises {
        if !exercise_names.contains(&exercise.name) {
            log::info!("Removed exercise {}", exercise.name);
            removed_exercises.push(exercise.name.clone());
        }
    }
    Ok((new_exercises, removed_exercises))
}

fn update_exercise_options(
    course_exercises: &[RefreshExercise],
    course_clone_path: &Path,
    course_name: &str,
) -> Result<ExerciseOptions, UtilError> {
    let exercise_default_metadata = {
        use Value::{Bool, Null, String};
        let mut defaults = Mapping::new();
        defaults.insert(String("deadline".to_string()), Null);
        defaults.insert(String("soft_deadline".to_string()), Null);
        defaults.insert(String("publish_time".to_string()), Null);
        defaults.insert(String("gdocs_sheet".to_string()), Null);
        defaults.insert(String("points_visible".to_string()), Bool(true));
        defaults.insert(String("hidden".to_string()), Bool(false));
        defaults.insert(String("returnable".to_string()), Null);
        defaults.insert(String("solution_visible_after".to_string()), Null);
        defaults.insert(
            String("valgrind_strategy".to_string()),
            String("fail".to_string()),
        );
        defaults.insert(String("runtime_params".to_string()), Null);
        defaults.insert(
            String("code_review_requests_enabled".to_string()),
            Bool(true),
        );
        defaults.insert(
            String("run_tests_locally_action_enabled".to_string()),
            Bool(true),
        );
        defaults
    };

    let mut review_points = HashMap::new();
    let mut metadata_map = HashMap::new();
    for exercise in course_exercises {
        let mut metadata = exercise_default_metadata.clone();
        let mut try_merge_metadata_in_dir = |path: &Path| -> Result<(), UtilError> {
            let metadata_path = path.join("metadata.yml");
            log::debug!("checking for metadata file {}", metadata_path.display());
            if metadata_path.exists() {
                let file = file_util::open_file(metadata_path)?;
                if let Ok(mut yaml) = serde_yaml::from_reader::<_, Mapping>(file) {
                    merge_course_specific_options(course_name, &mut yaml);
                    recursive_merge(yaml, &mut metadata);
                }
            }
            Ok(())
        };

        try_merge_metadata_in_dir(&course_clone_path)?;
        let mut rel_path = PathBuf::from(".");
        // traverse
        for component in exercise.relative_path.components() {
            rel_path = rel_path.join(component);
            try_merge_metadata_in_dir(&course_clone_path.join(&rel_path))?;
        }

        let exercise_review_points = match metadata.get(&Value::String("review_points".to_string()))
        {
            Some(Value::String(string)) => {
                string.split_whitespace().map(|s| s.to_string()).collect()
            }
            Some(Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect(),
            _ => vec![], // todo: empty vec is correct for null, but what to do with other values?
        };
        review_points.insert(exercise.name.clone(), exercise_review_points);
        metadata_map.insert(exercise.name.clone(), metadata);
    }

    Ok(ExerciseOptions {
        review_points,
        metadata_map,
    })
}

fn update_available_points(
    course_exercises: &[RefreshExercise],
    course_clone_path: &Path,
    review_points: &HashMap<String, Vec<String>>,
) -> Result<HashMap<String, UpdatePoints>, UtilError> {
    // scans the exercise directory and compares the points found (and review points) with what was given in the arguments
    // to find new and removed points
    let mut update_points = HashMap::new();
    for exercise in course_exercises {
        let review_points = review_points.get(&exercise.name).unwrap(); // safe, previous steps guarantee each exercise has review points
        let mut point_names = HashSet::new();

        let points_data = {
            // if options.no_directory_changes {
            //  optimization not implemented
            // }
            let path = course_clone_path.join(&exercise.relative_path);
            task_executor::get_available_points(&path)?
        };
        point_names.extend(points_data);
        point_names.extend(review_points.clone());

        let mut added = vec![];
        let mut removed = vec![];

        for name in &point_names {
            if !exercise.available_points.contains(name) {
                added.push(name.clone());
            }
        }

        for point in &exercise.available_points {
            if !point_names.contains(point) {
                removed.push(point.clone());
            }
        }

        if !added.is_empty() {
            log::info!(
                "Added points to exercise {}: {}",
                exercise.name,
                added.join(", ")
            );
        }
        if !removed.is_empty() {
            log::info!(
                "Removed points from exercise {}: {}",
                exercise.name,
                removed.join(", ")
            );
        }
        update_points.insert(exercise.name.clone(), UpdatePoints { added, removed });
    }
    Ok(update_points)
}

fn checksum_stubs(
    course_exercises: &[RefreshExercise],
    course_stub_path: &Path,
) -> Result<HashMap<String, Digest>, UtilError> {
    let mut checksum_stubs = HashMap::new();
    for e in course_exercises {
        let mut digest = Context::new();
        for entry in WalkDir::new(course_stub_path.join(&e.relative_path))
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .into_iter()
            .filter_entry(|e| {
                !e.file_name()
                    .to_str()
                    .map(|e| e.starts_with('.'))
                    .unwrap_or_default()
            })
        // filter hidden
        {
            let entry = entry?;
            if entry.path().is_file() {
                let relative = entry.path().strip_prefix(course_stub_path).unwrap(); // safe
                digest.consume(relative.as_os_str().to_string_lossy().into_owned());
                let file = file_util::read_file(dbg!(entry.path()))?;
                digest.consume(file);
            }
        }
        checksum_stubs.insert(e.name.clone(), digest.compute());
    }
    Ok(checksum_stubs)
}

fn execute_zip(
    course_exercises: &[RefreshExercise],
    root_path: &Path,
    zip_path: &Path,
) -> Result<(), UtilError> {
    file_util::create_dir_all(zip_path)?;
    for e in course_exercises {
        let mut stdin = String::new();
        let root = root_path.join(&e.relative_path);
        for entry in WalkDir::new(&root)
            .min_depth(1)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
        {
            let entry = entry?;
            let stub_path = entry.path().strip_prefix(&root).unwrap(); // safe
            stdin.push_str(&format!("{}\n", e.relative_path.join(stub_path).display()));
        }
        let zip_file_path = zip_path.join(format!("{}.zip", e.name));
        TmcCommand::new("zip")
            .with(|e| {
                e.arg("--quiet")
                    .arg("-@")
                    .arg(zip_file_path)
                    .cwd(&root_path)
                    .stdin(stdin.as_str())
            })
            .output_checked()?;
    }
    Ok(())
}

fn set_permissions(
    course_cache_path: &Path,
    chmod: Option<String>,
    chgrp: Option<String>,
    cache_root: &Path,
    rails_root: PathBuf,
) -> Result<(), UtilError> {
    // verify that the cache root is inside the rails root
    if !cache_root.starts_with(&rails_root) {
        return Err(UtilError::CacheNotInRailsRoot(
            cache_root.to_path_buf(),
            rails_root,
        ));
    };

    let run_chmod = |dir: &Path| -> Result<(), UtilError> {
        if let Some(chmod) = &chmod {
            TmcCommand::new("chmod")
                .with(|e| e.arg(chmod).cwd(dir))
                .output_checked()?;
        }
        Ok(())
    };
    let run_chgrp = |dir: &Path| -> Result<(), UtilError> {
        if let Some(chgrp) = &chgrp {
            TmcCommand::new("chgrp")
                .with(|e| e.arg(chgrp).cwd(dir))
                .output_checked()?;
        }
        Ok(())
    };

    // mod all directories from cache root up to rails root
    let mut next = cache_root;
    run_chmod(next)?;
    run_chgrp(next)?;
    while let Some(parent) = next.parent() {
        run_chmod(parent)?;
        run_chgrp(parent)?;
        if parent == rails_root {
            break;
        }
        next = parent;
    }

    if let Some(chmod) = &chmod {
        TmcCommand::new("chmod")
            .with(|e| e.arg("-R").arg(chmod).cwd(&course_cache_path))
            .output_checked()?;
    }
    if let Some(chgrp) = &chgrp {
        TmcCommand::new("chgrp")
            .with(|e| e.arg("-R").arg(chgrp).cwd(&course_cache_path))
            .output_checked()?;
    }
    Ok(())
}

fn recursive_merge(from: Mapping, into: &mut Mapping) {
    for (key, value) in from {
        if let Value::Mapping(from_mapping) = value {
            if let Some(Value::Mapping(into_mapping)) = into.get_mut(&key) {
                // combine mappings
                recursive_merge(from_mapping, into_mapping);
            } else {
                into.insert(key, Value::Mapping(from_mapping));
            }
        } else {
            into.insert(key.clone(), value);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const GIT_REPO: &'static str = "https://github.com/rage/rfcs";

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn updates_repository() {
        init();

        let cache = tempfile::TempDir::new().unwrap();
        file_util::create_dir_all(cache.path().join("clone")).unwrap();
        let run_git = |args: &[&str], cwd: &Path| {
            TmcCommand::new("git")
                .with(|e| {
                    e.args(args)
                        .cwd(cwd)
                        .stdout(Redirection::Pipe)
                        .stderr(Redirection::Pipe)
                })
                .output_checked()
                .unwrap()
        };
        run_git(&["init"], &cache.path().join("clone"));
        assert!(cache.path().join("clone/.git").exists());

        let clone = tempfile::TempDir::new().unwrap();
        run_git(&["init"], &clone.path());
        run_git(&["remote", "add", "origin", ""], &clone.path());

        let course = Course {
            name: "name".to_string(),
            cache_path: PathBuf::from(""),
            clone_path: clone.path().to_path_buf(),
            stub_path: PathBuf::from(""),
            stub_zip_path: PathBuf::from(""),
            solution_path: PathBuf::from(""),
            solution_zip_path: PathBuf::from(""),
            exercises: vec![],
            source_backend: SourceBackend::Git,
            source_url: GIT_REPO.to_string(),
            git_branch: "master".to_string(),
        };
        update_or_clone_repository(&course, cache.path()).unwrap();
        assert!(clone.path().join("texts").exists());
    }

    #[test]
    fn clones_repository() {
        init();

        let clone = tempfile::TempDir::new().unwrap();
        assert!(!clone.path().join(".git").exists());
        let course = Course {
            name: "name".to_string(),
            cache_path: PathBuf::from(""),
            clone_path: clone.path().to_path_buf(),
            stub_path: PathBuf::from(""),
            stub_zip_path: PathBuf::from(""),
            solution_path: PathBuf::from(""),
            solution_zip_path: PathBuf::from(""),
            exercises: vec![],
            source_backend: SourceBackend::Git,
            source_url: GIT_REPO.to_string(),
            git_branch: "master".to_string(),
        };
        let old_cache_path = Path::new("nonexistent");
        update_or_clone_repository(&course, old_cache_path).unwrap();
        assert!(clone.path().join("texts").exists());
    }

    #[test]
    fn checks_directory_names() {
        init();

        assert!(
            check_directory_names(Path::new("tests/data/course_refresher/valid_exercises")).is_ok()
        );
        assert!(
            check_directory_names(Path::new("tests/data/course_refresher/invalid_exercises"))
                .is_err()
        );
    }

    #[test]
    fn updates_course_options_empty() {
        init();

        let clone = tempfile::TempDir::new().unwrap();
        let name = "name";
        let options = update_course_options(clone.path(), name).unwrap();
        assert!(options.is_empty());
    }

    #[test]
    fn updates_course_options_non_empty() {
        init();

        let clone_path = Path::new("tests/data/course_refresher");
        let name = "course-name";
        let options = update_course_options(clone_path, name).unwrap();
        assert!(!options.is_empty());

        let b = options
            .get(&Value::String("inner_value".to_string()))
            .unwrap()
            .as_bool()
            .unwrap();
        assert!(b);

        let val = options
            .get(&Value::String("inner_map".to_string()))
            .unwrap()
            .as_mapping()
            .unwrap();
        let val = val
            .get(&Value::String("param".to_string()))
            .unwrap()
            .as_i64()
            .unwrap();
        assert_eq!(val, 1)
    }

    #[test]
    fn updates_exercises() {
        init();

        let clone_path = Path::new("tests/data/course_refresher/empty");
        let (new, removed) = update_exercises(clone_path, &[]).unwrap();
        assert!(new.is_empty());
        assert!(removed.is_empty());

        let clone_path = Path::new("tests/data/course_refresher/valid_exercises");
        let (new, removed) = update_exercises(
            clone_path,
            &[
                RefreshExercise {
                    available_points: vec![],
                    relative_path: PathBuf::new(),
                    name: "ex2".to_string(),
                },
                RefreshExercise {
                    available_points: vec![],
                    relative_path: PathBuf::new(),
                    name: "ex3".to_string(),
                },
            ],
        )
        .unwrap();
        assert_eq!(new, &["ex1"]);
        assert_eq!(removed, &["ex3"]);
    }

    #[test]
    fn updates_exercise_options() {
        init();

        let opts = update_exercise_options(&[], Path::new(""), "").unwrap();
        assert!(opts.review_points.is_empty());
        assert!(opts.metadata_map.is_empty());

        let opts = dbg!(update_exercise_options(
            &[
                RefreshExercise {
                    available_points: vec![],
                    name: "defaults".to_string(),
                    relative_path: PathBuf::from("ex1"),
                },
                RefreshExercise {
                    available_points: vec![],
                    name: "deep".to_string(),
                    relative_path: PathBuf::from("deep/deeper"),
                },
            ],
            Path::new("tests/data/course_refresher/valid_exercises"),
            "course-name-PART1",
        )
        .unwrap());
        assert!(!opts.review_points.is_empty());
        assert_eq!(opts.metadata_map.len(), 2);

        let val = opts.metadata_map.get("defaults").unwrap();
        assert!(val
            .get(&Value::String("points_visible".to_string()))
            .unwrap()
            .as_bool()
            .unwrap());

        let val = opts.metadata_map.get("deep").unwrap();
        assert!(!val
            .get(&Value::String("points_visible".to_string()))
            .unwrap()
            .as_bool()
            .unwrap());
        assert!(val
            .get(&Value::String("true_in_inner_false_in_outer".to_string()))
            .unwrap()
            .as_bool()
            .unwrap());
        let inner_map = val
            .get(&Value::String("inner_map".to_string()))
            .unwrap()
            .as_mapping()
            .unwrap();
        assert!(inner_map
            .get(&Value::String("true_in_inner_false_in_outer".to_string()))
            .unwrap()
            .as_bool()
            .unwrap());
    }

    #[test]
    fn updates_available_points() {
        let mut review_points = HashMap::new();
        review_points.insert(
            "ex1".to_string(),
            vec!["rev_point".to_string(), "ex_and_rev_point".to_string()],
        );

        let update_points = dbg!(update_available_points(
            &[RefreshExercise {
                available_points: vec![
                    "ex_point".to_string(),
                    "ex_and_rev_point".to_string(),
                    "ex_and_test_point".to_string()
                ],
                name: "ex1".to_string(),
                relative_path: PathBuf::from("ex1"),
            }],
            Path::new("tests/data/course_refresher/valid_exercises"),
            &review_points,
        )
        .unwrap());
        let pts = update_points.get("ex1").unwrap();
        assert_eq!(pts.added, &["rev_point", "test_point"]);
        assert_eq!(pts.removed, &["ex_point"]);
    }

    #[test]
    fn checksums_stubs() {
        let first = tempfile::tempdir().unwrap();
        std::fs::create_dir(first.path().join("dir")).unwrap();
        std::fs::write(first.path().join("dir/file"), b"hello").unwrap();
        std::fs::write(first.path().join("dir/.hidden"), b"hello").unwrap();

        let second = tempfile::tempdir().unwrap();
        std::fs::create_dir(second.path().join("dir")).unwrap();
        std::fs::write(second.path().join("dir/file"), b"hello").unwrap();
        std::fs::write(second.path().join("dir/.hidden"), b"bye").unwrap();

        let third = tempfile::tempdir().unwrap();
        std::fs::create_dir(third.path().join("dir")).unwrap();
        std::fs::write(third.path().join("dir/file"), b"bye").unwrap();

        let cks = dbg!(checksum_stubs(
            &[RefreshExercise {
                available_points: vec![],
                name: "first".to_string(),
                relative_path: PathBuf::from("dir")
            }],
            first.path(),
        )
        .unwrap());
        let f = cks.get("first").unwrap();

        let cks = dbg!(checksum_stubs(
            &[RefreshExercise {
                available_points: vec![],
                name: "second".to_string(),
                relative_path: PathBuf::from("dir")
            }],
            second.path(),
        )
        .unwrap());
        let s = cks.get("second").unwrap();

        let cks = dbg!(checksum_stubs(
            &[RefreshExercise {
                available_points: vec![],
                name: "third".to_string(),
                relative_path: PathBuf::from("dir")
            }],
            third.path(),
        )
        .unwrap());
        let t = cks.get("third").unwrap();

        assert_eq!(f, s);
        assert_ne!(f, t);
    }
}