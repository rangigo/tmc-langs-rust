//! Contains the RPlugin

use crate::error::RError;
use crate::RRunResult;
use crate::RStudentFilePolicy;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tmc_langs_framework::{
    anyhow,
    command::TmcCommand,
    domain::{ExerciseDesc, RunResult, TestDesc},
    io::file_util,
    nom::{bytes, character, combinator, sequence, IResult},
    zip::ZipArchive,
    LanguagePlugin, TmcError,
};

#[derive(Default)]
pub struct RPlugin {}

impl RPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

impl LanguagePlugin for RPlugin {
    const PLUGIN_NAME: &'static str = "r";
    const LINE_COMMENT: &'static str = "#";
    const BLOCK_COMMENT: Option<(&'static str, &'static str)> = None;
    type StudentFilePolicy = RStudentFilePolicy;

    fn scan_exercise(
        &self,
        path: &Path,
        exercise_name: String,
        _warnings: &mut Vec<anyhow::Error>,
    ) -> Result<ExerciseDesc, TmcError> {
        // run available points command
        let args = if cfg!(windows) {
            &["-e", "\"library('tmcRtestrunner');run_available_points()\""]
        } else {
            &["-e", "library(tmcRtestrunner);run_available_points()"]
        };
        let _output = TmcCommand::new_with_file_io("Rscript")?
            .with(|e| e.cwd(path).args(args))
            .output_checked()?;

        // parse exercise desc
        let points_path = path.join(".available_points.json");
        let json_file = file_util::open_file(&points_path)?;
        let test_descs: HashMap<String, Vec<String>> = serde_json::from_reader(json_file)
            .map_err(|e| RError::JsonDeserialize(points_path, e))?;
        let test_descs = test_descs
            .into_iter()
            .map(|(k, v)| TestDesc { name: k, points: v })
            .collect();

        Ok(ExerciseDesc {
            name: exercise_name,
            tests: test_descs,
        })
    }

    fn run_tests_with_timeout(
        &self,
        path: &Path,
        _timeout: Option<Duration>,
        _warnings: &mut Vec<anyhow::Error>,
    ) -> Result<RunResult, TmcError> {
        // delete results json
        let results_path = path.join(".results.json");
        if results_path.exists() {
            file_util::remove_file(&results_path)?;
        }

        // run test command
        let args = if cfg!(windows) {
            &["-e", "\"library('tmcRtestrunner');run_tests()\""]
        } else {
            &["-e", "library(tmcRtestrunner);run_tests()"]
        };
        let _command = TmcCommand::new_with_file_io("Rscript")?
            .with(|e| e.cwd(path).args(args))
            .output_checked()?;

        // parse test result
        let json_file = file_util::open_file(&results_path)?;
        let run_result: RRunResult = serde_json::from_reader(json_file).map_err(|e| {
            if let Ok(s) = fs::read_to_string(&results_path) {
                log::error!("json {}", s);
            }
            RError::JsonDeserialize(results_path, e)
        })?;

        Ok(run_result.into())
    }

    fn get_student_file_policy(project_path: &Path) -> Self::StudentFilePolicy {
        RStudentFilePolicy::new(project_path.to_path_buf())
    }

    /// Checks if the directory contains R or tests/testthat
    fn is_exercise_type_correct(path: &Path) -> bool {
        path.join("R").exists() || path.join("tests/testthat").exists()
    }

    /// Finds an R directory.
    /// Ignores everything in a __MACOSX directory.
    fn find_project_dir_in_zip<R: Read + Seek>(
        zip_archive: &mut ZipArchive<R>,
    ) -> Result<PathBuf, TmcError> {
        for i in 0..zip_archive.len() {
            // zips don't necessarily contain entries for intermediate directories,
            // so we need to check every path for R
            let file = zip_archive.by_index(i)?;
            let file_path = Path::new(file.name());

            // todo: do in one pass somehow
            if file_path.components().any(|c| c.as_os_str() == "R") {
                let path: PathBuf = file_path
                    .components()
                    .take_while(|c| c.as_os_str() != "R")
                    .collect();

                if path.components().any(|p| p.as_os_str() == "__MACOSX") {
                    continue;
                }
                return Ok(path);
            }
        }
        Err(TmcError::NoProjectDirInZip)
    }

    /// No operation for now. To be possibly implemented later: remove .Rdata, .Rhistory etc
    fn clean(&self, _path: &Path) -> Result<(), TmcError> {
        Ok(())
    }

    fn get_default_student_file_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("R")]
    }

    fn get_default_exercise_file_paths(&self) -> Vec<PathBuf> {
        vec![PathBuf::from("tests")]
    }

    fn points_parser<'a>(i: &'a str) -> IResult<&'a str, &'a str> {
        combinator::map(
            sequence::delimited(
                sequence::tuple((
                    bytes::complete::tag("test"),
                    character::complete::multispace0,
                    character::complete::char('('),
                    bytes::complete::take_until(","),
                    bytes::complete::take_until("\""),
                )),
                sequence::delimited(
                    character::complete::char('"'),
                    bytes::complete::is_not("\""),
                    character::complete::char('"'),
                ),
                sequence::tuple((
                    character::complete::multispace0,
                    character::complete::char(')'),
                )),
            ),
            str::trim,
        )(i)
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")] // tmc-r-testrunner not installed on other CI platforms
mod test {
    use super::*;
    use std::fs::File;
    use std::path::PathBuf;
    use tmc_langs_framework::domain::RunStatus;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    // copies the target exercise and tmc to a temp directory
    fn copy_test(dir: &str) -> tempfile::TempDir {
        let path = Path::new(dir);
        let temp = tempfile::tempdir().unwrap();
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                let entry_path: PathBuf = entry
                    .path()
                    .components()
                    .skip(path.components().count())
                    .collect();
                let temp_path = temp.path().join(entry_path);
                if let Some(parent) = temp_path.parent() {
                    std::fs::create_dir_all(&parent).unwrap();
                }
                log::trace!("copying {:?} -> {:?}", entry.path(), temp_path);
                std::fs::copy(entry.path(), temp_path).unwrap();
            }
        }
        temp
    }

    #[test]
    fn scan_exercise() {
        init();
        let plugin = RPlugin {};
        let temp = copy_test("tests/data/simple_all_tests_pass");

        assert!(!temp.path().join(".available_points.json").exists());
        let desc = plugin
            .scan_exercise(temp.path(), "ex".to_string(), &mut vec![])
            .unwrap();
        assert!(temp.path().join(".available_points.json").exists());
        assert_eq!(desc.name, "ex");
        assert_eq!(desc.tests.len(), 4);
        for test in desc.tests {
            if test.name == "ret_true works." {
                assert_eq!(test.points.len(), 2);
                assert_eq!(test.points[0], "r1");
                return;
            }
        }
        panic!("not found");
    }

    #[test]
    fn run_tests_success() {
        init();
        let plugin = RPlugin {};
        let temp = copy_test("tests/data/simple_all_tests_pass");

        let run = plugin.run_tests(temp.path(), &mut vec![]).unwrap();
        assert_eq!(run.status, RunStatus::Passed);
        assert!(run.logs.is_empty());
        assert_eq!(run.test_results.len(), 4);
        for res in run.test_results {
            if res.name == "ret_true works." {
                assert!(res.successful);
                assert_eq!(res.points, &["r1", "r1.1"]);
                assert!(res.message.is_empty());
                assert!(res.exception.is_empty());
                return;
            }
        }
        panic!("not found");
    }

    #[test]
    fn run_tests_failed() {
        init();
        let plugin = RPlugin {};
        let temp = copy_test("tests/data/simple_all_tests_fail");

        let run = plugin.run_tests(temp.path(), &mut vec![]).unwrap();
        assert_eq!(run.status, RunStatus::TestsFailed);
        assert!(run.logs.is_empty());
        assert_eq!(run.test_results.len(), 4);
        for res in run.test_results {
            if res.name == "ret_true works." {
                assert!(!res.successful);
                assert_eq!(res.points, &["r1", "r1.1"]);
                assert!(!res.message.is_empty());
                assert!(res.exception.is_empty());
                return;
            }
        }
        panic!("not found");
    }

    #[test]
    fn run_tests_run_failed() {
        init();
        let plugin = RPlugin {};
        let temp = copy_test("tests/data/simple_run_fail");

        let mut run = plugin.run_tests(temp.path(), &mut vec![]).unwrap();
        assert_eq!(run.status, RunStatus::CompileFailed);
        assert!(run.test_results.is_empty());
        assert!(!run.logs.is_empty());
        let logs = run.logs.remove("compiler_output").unwrap();
        assert!(logs.contains("unexpected 'in'"))
    }

    #[test]
    fn run_tests_sourcing() {
        init();
        let plugin = RPlugin {};
        let temp = copy_test("tests/data/simple_sourcing_fail");

        let mut run = plugin.run_tests(temp.path(), &mut vec![]).unwrap();
        assert_eq!(run.status, RunStatus::CompileFailed);
        assert!(run.test_results.is_empty());
        assert!(!run.logs.is_empty());
        let logs = run.logs.remove("compiler_output").unwrap();
        assert!(logs.contains("unexpected 'in'"));
    }

    #[test]
    fn finds_project_dir_in_zip() {
        let file = File::open("tests/data/RProject.zip").unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let dir = RPlugin::find_project_dir_in_zip(&mut zip).unwrap();
        assert_eq!(dir, Path::new("Outer/Inner/simple_all_tests_pass"));
    }

    #[test]
    fn doesnt_find_project_dir_in_zip() {
        let file = File::open("tests/data/RWithoutR.zip").unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let dir = RPlugin::find_project_dir_in_zip(&mut zip);
        assert!(dir.is_err());
    }

    #[test]
    fn parses_points() {
        let target = "asd";
        assert!(RPlugin::points_parser(target).is_err());

        let target = "test ( \"first arg\", \"second arg but no brace\"";
        assert!(RPlugin::points_parser(target).is_err());

        let target = r#"test("1d and 1e are solved correctly", c("W1A.1.2"), {
  expect_equivalent(z, z_correct, tolerance=1e-5)
  expect_true(areEqual(res, res_correct))
})
"#;
        assert_eq!(RPlugin::points_parser(target).unwrap().1, "W1A.1.2");
    }
}
