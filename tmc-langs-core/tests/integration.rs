//! Integration tests using the courses from TMC's test organization
//! Requires EMAIL and PASSWORD to be defined in tmc-langs-core/.env

use dotenv::dotenv;
use std::env;
use std::path::Path;
use std::thread;
use std::time::Duration;
use tmc_langs_core::{
    CoreError, Exercise, SubmissionFinished, SubmissionProcessingStatus, TmcCore,
};
use tmc_langs_util::{Language, RunStatus};
use url::Url;

const TMC_ROOT: &str = "https://tmc.mooc.fi/";

fn init() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "debug,j4rs=warn,hyper=warn,reqwest=warn");
    }
    let _ = env_logger::builder().is_test(true).try_init();
    dotenv().ok();
}

fn authenticated_core() -> TmcCore {
    let email = env::var("EMAIL").unwrap();
    let password = env::var("PASSWORD").unwrap();
    let mut core = TmcCore::new_in_config(TMC_ROOT.to_string()).unwrap();
    core.authenticate("vscode_plugin", email, password).unwrap();
    core
}

// downloads and submits all exercises for course, asserts that the tests are run but fail
fn dl_test_submit_course_templates(course_id: usize) {
    init();

    fn submitter(core: &TmcCore, exercise: Exercise) -> SubmissionFinished {
        let id = exercise.id;
        dl_test_submit_exercise(
            &core,
            exercise,
            |target| core.download_or_update_exercises(vec![(id, target)]),
            false,
        )
    }

    dl_test_submit_course_exercises(course_id, submitter, false);
}

// downloads and submits all exercise solutions for course, asserts that tests pass
fn dl_test_submit_course_solutions(course_id: usize) {
    init();

    fn submitter(core: &TmcCore, exercise: Exercise) -> SubmissionFinished {
        let solution_url = Url::parse(&exercise.return_url)
            .unwrap()
            .join("solution/download")
            .unwrap();
        dl_test_submit_exercise(
            &core,
            exercise,
            |target| core.download_model_solution(solution_url, target),
            true,
        )
    }

    dl_test_submit_course_exercises(course_id, submitter, true);
}

// fetches course exercises and runs submitter on each one
// tester_submitter should test and submit the exercise
fn dl_test_submit_course_exercises<F>(course_id: usize, tester_submitter: F, should_pass: bool)
where
    F: Fn(&TmcCore, Exercise) -> SubmissionFinished,
{
    log::debug!("fetching course {}", course_id);
    let core = authenticated_core();
    let course_details = core.get_course_details(course_id).unwrap();
    log::debug!(
        "testing and submitting course templates for {:#?}",
        course_details
    );

    for exercise in course_details.exercises {
        if exercise.name.contains("osa01")
            || exercise.name.contains("osa02")
            || exercise.name.contains("osa03")
            || exercise.name.contains("osa04")
            || exercise.name.contains("osa05")
        {
            // temp, these parts have been checked already for java
            continue;
        }
        if [93659, 92964, 92960, 82587].contains(&exercise.id) {
            log::info!("skipping {}: solution does not pass tests", exercise.id);
            continue;
        }
        if [94570, 94726].contains(&exercise.id) {
            log::info!("skipping {}: template always passes", exercise.id);
            continue;
        }

        let finished = tester_submitter(&core, exercise);
        log::debug!("finished {:#?}", finished);
        assert_eq!(should_pass, finished.all_tests_passed.unwrap());
    }
}

// submits the exercise
// downloader should download the submission target to the path arg
fn dl_test_submit_exercise<F: FnOnce(&Path) -> Result<(), CoreError>>(
    core: &TmcCore,
    exercise: Exercise,
    downloader: F,
    should_pass: bool,
) -> SubmissionFinished {
    log::debug!("submitting exercise {:#?}", exercise);
    let temp = tempfile::tempdir().unwrap();
    let submission_path = temp.path().join(exercise.id.to_string());
    log::debug!("downloading to {}", submission_path.display());
    downloader(&submission_path).unwrap();

    log::debug!("testing locally {}", submission_path.display());
    let test_results = core.run_tests(&submission_path).unwrap();
    if should_pass {
        assert_eq!(test_results.status, RunStatus::Passed);
    } else {
        assert_eq!(test_results.status, RunStatus::TestsFailed);
    }

    let submission_url = Url::parse(&exercise.return_url).unwrap();
    log::debug!("submitting to {}", submission_url);
    let submission = core
        .submit(submission_url, &submission_path, Language::Eng)
        .unwrap();
    log::debug!("got {:#?}", submission);

    log::debug!("waiting for submission to finish");
    let finished = loop {
        let status = core.check_submission(&submission.submission_url).unwrap();
        match status {
            SubmissionProcessingStatus::Finished(finished) => break *finished,
            SubmissionProcessingStatus::Processing(_) => thread::sleep(Duration::from_secs(2)),
        }
    };
    log::debug!("got {:#?}", finished);
    finished
}

mod python {
    use super::*;

    const PYTHON_COURSE_ID: usize = 597;

    #[test]
    #[ignore]
    // passed 29.6.2020
    fn templates() {
        dl_test_submit_course_templates(PYTHON_COURSE_ID)
    }

    #[test]
    #[ignore]
    // passed 29.6.2020
    fn solutions() {
        dl_test_submit_course_solutions(PYTHON_COURSE_ID)
    }
}

mod java {
    use super::*;

    const JAVA_COURSE_ID: usize = 665;

    #[test]
    #[ignore]
    fn templates() {
        dl_test_submit_course_templates(JAVA_COURSE_ID)
    }

    #[test]
    #[ignore]
    fn solutions() {
        dl_test_submit_course_solutions(JAVA_COURSE_ID)
    }
}

mod csharp {
    use super::*;

    const CSHARP_COURSE_ID: usize = 651;

    #[test]
    #[ignore]
    fn templates() {
        dl_test_submit_course_templates(CSHARP_COURSE_ID)
    }

    #[test]
    #[ignore]
    fn solutions() {
        dl_test_submit_course_solutions(CSHARP_COURSE_ID)
    }
}
