// End-to-end tests that drive the real binary against a throwaway log file.
//
// These cover one or two cases per subcommand rather than every branch: the
// point is to catch a command that stops producing output, stops writing to the
// log, or stops emitting parseable JSON.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

// a bartib log file that deletes itself when the test ends
struct TestLog {
    path: PathBuf,
}

impl TestLog {
    fn with_content(content: &str) -> TestLog {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "bartib-test-{}-{}.bartib",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));

        fs::write(&path, content).expect("could not write test log");

        TestLog { path }
    }

    fn empty() -> TestLog {
        TestLog::with_content("")
    }

    // three stopped activities across two projects, with a repeated description
    fn sample() -> TestLog {
        TestLog::with_content(concat!(
            "2026-08-25 09:00 - 2026-08-25 10:30 | proj_a | task one\n",
            "2026-08-25 11:00 - 2026-08-25 12:00 | proj_b | task two\n",
            "2026-08-25 13:00 - 2026-08-25 14:00 | proj_a | task one\n",
        ))
    }

    fn read(&self) -> String {
        fs::read_to_string(&self.path).expect("could not read test log")
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bartib"))
            .arg("-f")
            .arg(&self.path)
            .args(args)
            .output()
            .expect("could not run bartib")
    }

    // runs the command and asserts it succeeded, returning stdout
    fn stdout(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "`bartib {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("stdout was not utf8")
    }
}

impl Drop for TestLog {
    fn drop(&mut self) {
        // the read-only test may leave the file unwritable
        if let Ok(metadata) = fs::metadata(&self.path) {
            let mut permissions = metadata.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(&self.path, permissions);
        }
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn start_writes_a_running_activity() {
    let log = TestLog::empty();
    log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

    let content = log.read();
    assert!(content.contains("09:00 | proj | desc"), "got: {}", content);
    // a running activity has no end time, so no " - " separator
    assert!(!content.contains(" - "), "got: {}", content);
}

#[test]
fn stop_sets_an_end_time() {
    let log = TestLog::empty();
    log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);
    log.stdout(&["stop", "-t", "10:00"]);

    let content = log.read();
    assert!(content.contains("09:00 - "), "got: {}", content);
    assert!(content.contains("10:00 | proj | desc"), "got: {}", content);
}

#[test]
fn stop_reports_write_failure_instead_of_claiming_success() {
    // the click-to-pause path of a status bar widget: if the log cannot be
    // written, the command must fail rather than print "stopped" and exit 0
    #[cfg(unix)]
    {
        let log = TestLog::empty();
        log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

        let mut permissions = fs::metadata(&log.path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&log.path, permissions).unwrap();

        let output = log.run(&["toggle"]);
        assert!(!output.status.success(), "toggle hid a failed write");
        assert!(String::from_utf8_lossy(&output.stderr).contains("Could not write to file"));
    }
}

#[test]
fn cancel_discards_the_running_activity() {
    let log = TestLog::empty();
    log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);
    log.stdout(&["cancel"]);

    assert!(log.read().trim().is_empty(), "got: {}", log.read());
}

#[test]
fn change_updates_the_running_activity() {
    let log = TestLog::empty();
    log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);
    log.stdout(&["change", "-p", "other", "-d", "renamed"]);

    let content = log.read();
    assert!(content.contains("other"), "got: {}", content);
    assert!(content.contains("renamed"), "got: {}", content);
}

#[test]
fn continue_restarts_the_last_activity() {
    let log = TestLog::sample();
    log.stdout(&["continue", "-t", "15:00"]);

    let content = log.read();
    // the most recently started activity was "task one" on proj_a
    assert!(
        content.contains("15:00 | proj_a | task one"),
        "got: {}",
        content
    );
}

#[test]
fn continue_addresses_activities_by_zero_based_index() {
    // the sample log holds two distinct activities, so 0 and 1 are the only
    // valid numbers
    let log = TestLog::sample();
    log.stdout(&["continue", "1", "-t", "15:00"]);

    assert!(
        log.read().contains("15:00 | proj_b | task two"),
        "got: {}",
        log.read()
    );
}

#[test]
fn continue_rejects_an_index_past_the_end() {
    // `continue 2` with two distinct activities used to saturate to index 0 and
    // silently continue the oldest entry instead of reporting the mistake
    let log = TestLog::sample();
    let output = log.run(&["continue", "2", "-t", "15:00"]);

    assert!(!output.status.success(), "continue 2 should have failed");
    assert!(String::from_utf8_lossy(&output.stderr).contains("distinct activities"));
    assert!(
        !log.read().contains("15:00"),
        "log was modified: {}",
        log.read()
    );
}

#[test]
fn file_can_be_given_as_a_long_flag() {
    // the documentation refers to --file, not just -f
    let log = TestLog::sample();
    let output = Command::new(env!("CARGO_BIN_EXE_bartib"))
        .arg("--file")
        .arg(&log.path)
        .arg("projects")
        .output()
        .expect("could not run bartib");

    assert!(output.status.success(), "--file was rejected");
    assert!(String::from_utf8_lossy(&output.stdout).contains("proj_a"));
}

#[test]
fn toggle_resumes_when_nothing_is_running() {
    let log = TestLog::sample();
    log.stdout(&["toggle", "-t", "15:00"]);

    let content = log.read();
    assert!(
        content.contains("15:00 | proj_a | task one"),
        "got: {}",
        content
    );
}

#[test]
fn toggle_stops_when_something_is_running() {
    let log = TestLog::sample();
    log.stdout(&["toggle", "-t", "15:00"]);
    log.stdout(&["toggle", "-t", "16:00"]);

    let content = log.read();
    assert!(
        content.contains("15:00 - 2026-08-26 16:00") || content.contains("15:00 - "),
        "got: {}",
        content
    );
    // nothing is left running: every line has an end time
    assert!(
        content.lines().all(|line| line.contains(" - ")),
        "an activity is still running: {}",
        content
    );
}

#[test]
fn check_reports_unparseable_lines() {
    let log = TestLog::with_content("this is not an activity\n");
    let out = log.stdout(&["check"]);

    assert!(
        out.contains("1 line(s) with parsing errors"),
        "got: {}",
        out
    );
}

#[test]
fn sanity_still_detects_a_negative_duration() {
    // clamping the reported duration must not hide an inconsistent log entry
    let log = TestLog::with_content("2026-08-25 10:00 - 2026-08-25 09:00 | proj | backwards\n");
    let out = log.stdout(&["sanity"]);

    assert!(out.contains("negative duration"), "got: {}", out);
}

#[test]
fn sanity_accepts_a_clean_log() {
    let log = TestLog::sample();
    let out = log.stdout(&["sanity"]);

    assert!(out.contains("No unusual activities"), "got: {}", out);
}

#[test]
fn warnings_go_to_stderr_so_stdout_stays_parseable() {
    // the reason --nowarn was removed: a warning on stdout corrupts any
    // downstream parser
    let log = TestLog::with_content(concat!(
        "2026-08-25 09:00 - 2026-08-25 10:30 | proj_a | task one\n",
        "this line is garbage\n",
    ));

    let output = log.run(&["projects"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("Ignoring line 2"), "stderr was: {}", stderr);
    assert!(!stdout.contains("Ignoring line"), "stdout was: {}", stdout);
}

// plaintext output
mod plaintext {
    use super::TestLog;

    #[test]
    fn current_lists_running_activities() {
        let log = TestLog::empty();
        log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

        let out = log.stdout(&["current"]);
        assert!(out.contains("desc"), "got: {}", out);
        assert!(out.contains("proj"), "got: {}", out);
    }

    #[test]
    fn current_says_so_when_idle() {
        let log = TestLog::sample();
        let out = log.stdout(&["current"]);

        assert!(
            out.contains("No Activity is currently running"),
            "got: {}",
            out
        );
    }

    #[test]
    fn list_shows_every_activity() {
        let log = TestLog::sample();
        let out = log.stdout(&["list"]);

        assert!(out.contains("task one"), "got: {}", out);
        assert!(out.contains("task two"), "got: {}", out);
    }

    #[test]
    fn report_totals_by_project() {
        let log = TestLog::sample();
        let out = log.stdout(&["report"]);

        assert!(out.contains("proj_a"), "got: {}", out);
        // 1h30m + 1h on proj_a
        assert!(out.contains("2h 30m"), "got: {}", out);
        assert!(out.contains("Total"), "got: {}", out);
    }

    #[test]
    fn projects_can_drop_the_quotes() {
        let log = TestLog::sample();

        assert!(log.stdout(&["projects"]).contains("\"proj_a\""));
        let bare = log.stdout(&["projects", "--no-quotes"]);
        assert!(
            bare.contains("proj_a") && !bare.contains('"'),
            "got: {}",
            bare
        );
    }

    #[test]
    fn last_lists_distinct_activities() {
        let log = TestLog::sample();
        let out = log.stdout(&["last"]);

        assert!(out.contains("task one"), "got: {}", out);
        assert!(out.contains("task two"), "got: {}", out);
    }

    #[test]
    fn search_filters_by_term() {
        let log = TestLog::sample();
        let out = log.stdout(&["search", "two"]);

        assert!(out.contains("task two"), "got: {}", out);
        assert!(!out.contains("task one"), "got: {}", out);
    }

    #[test]
    fn status_reports_the_current_activity() {
        let log = TestLog::empty();
        log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

        let out = log.stdout(&["status"]);
        assert!(out.contains("desc"), "got: {}", out);
    }
}

// json output
//
// these assert the shape a script or widget depends on: durations are numbers,
// timestamps are ISO 8601, and every command emits a single valid document.
#[cfg(feature = "json")]
mod json {
    use super::TestLog;
    use serde_json::Value;

    fn parse(text: &str) -> Value {
        serde_json::from_str(text)
            .unwrap_or_else(|e| panic!("not valid json: {}\ngot: {}", e, text))
    }

    #[test]
    fn current_emits_an_array_of_running_activities() {
        let log = TestLog::empty();
        log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

        let v = parse(&log.stdout(&["current", "--json"]));
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["project"], "proj");
        assert_eq!(v[0]["description"], "desc");
        assert_eq!(v[0]["end"], Value::Null);
        assert_eq!(v[0]["is_running"], true);
    }

    #[test]
    fn current_emits_an_empty_array_when_idle() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["current", "--json"]));

        assert_eq!(v.as_array().unwrap().len(), 0);
    }

    #[test]
    fn list_emits_iso_timestamps_and_numeric_durations() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["list", "--json"]));

        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[0]["start"], "2026-08-25T09:00:00");
        assert_eq!(v[0]["end"], "2026-08-25T10:30:00");
        // 1h30m, as a number rather than a display string
        assert_eq!(v[0]["duration_seconds"], 5400);
        assert_eq!(v[0]["is_running"], false);
    }

    #[test]
    fn report_nests_descriptions_under_projects() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["report", "--json"]));

        assert_eq!(v["total_duration_seconds"], 9000 + 3600);

        let projects = v["projects"].as_array().unwrap();
        assert_eq!(projects.len(), 2);

        let proj_a = &projects[0];
        assert_eq!(proj_a["project"], "proj_a");
        // both "task one" entries roll up into one description
        assert_eq!(proj_a["duration_seconds"], 9000);
        assert_eq!(proj_a["descriptions"].as_array().unwrap().len(), 1);
        assert_eq!(proj_a["descriptions"][0]["duration_seconds"], 9000);
    }

    #[test]
    fn projects_emits_a_flat_array_of_strings() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["projects", "--json"]));

        assert_eq!(v, serde_json::json!(["proj_a", "proj_b"]));
    }

    #[test]
    fn last_emits_indexed_entries() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["last", "--json"]));

        let entries = v.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        // index 0 is the most recently started, matching `continue 0`
        let newest = entries.iter().find(|e| e["index"] == 0).unwrap();
        assert_eq!(newest["description"], "task one");
        assert_eq!(newest["project"], "proj_a");
    }

    #[test]
    fn search_emits_only_matching_entries() {
        let log = TestLog::sample();
        let v = parse(&log.stdout(&["search", "two", "--json"]));

        let entries = v.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["description"], "task two");
    }

    #[test]
    fn status_emits_numeric_totals() {
        let log = TestLog::empty();
        log.stdout(&["start", "-p", "proj", "-d", "desc", "-t", "09:00"]);

        let v = parse(&log.stdout(&["status", "--json"]));

        assert_eq!(v["activity"]["description"], "desc");
        // the widget reads these, so they must be numbers, never "<1m"
        for key in [
            "today_seconds",
            "current_week_seconds",
            "current_month_seconds",
        ] {
            assert!(v[key].is_number(), "{} was not a number: {}", key, v[key]);
        }
    }

    #[test]
    fn durations_are_never_negative() {
        // a future-dated start would otherwise report negative elapsed time,
        // which a widget would render as a negative clock
        let log = TestLog::with_content("2026-08-25 10:00 - 2026-08-25 09:00 | proj | backwards\n");

        let v = parse(&log.stdout(&["list", "--json"]));
        assert_eq!(v[0]["duration_seconds"], 0);

        let report = parse(&log.stdout(&["report", "--json"]));
        assert_eq!(report["total_duration_seconds"], 0);
    }

    #[test]
    fn warnings_do_not_corrupt_the_json_document() {
        let log = TestLog::with_content(concat!(
            "2026-08-25 09:00 - 2026-08-25 10:30 | proj_a | task one\n",
            "this line is garbage\n",
        ));

        let output = log.run(&["list", "--json"]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(String::from_utf8_lossy(&output.stderr).contains("Ignoring line 2"));
        // stdout on its own must still parse
        let v = parse(&stdout);
        assert_eq!(v.as_array().unwrap().len(), 1);
    }
}
