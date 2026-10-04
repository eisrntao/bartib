use anyhow::Result;

use crate::conf;
use crate::data::activity::Activity;
use crate::data::processor::StatusReportData;
use crate::view::format_util;
use crate::view::list;
use crate::view::report;

// what happened to a single activity during a manipulation subcommand
// (start, change, stop, cancel, continue, toggle)
pub enum ActivityEventKind {
    Started,
    Stopped,
    Changed,
    Canceled,
}

pub struct ActivityEvent {
    pub kind: ActivityEventKind,
    pub activity: Activity,
}

impl ActivityEvent {
    #[must_use]
    pub fn started(activity: Activity) -> Self {
        Self { kind: ActivityEventKind::Started, activity }
    }

    #[must_use]
    pub fn stopped(activity: Activity) -> Self {
        Self { kind: ActivityEventKind::Stopped, activity }
    }

    #[must_use]
    pub fn changed(activity: Activity) -> Self {
        Self { kind: ActivityEventKind::Changed, activity }
    }

    #[must_use]
    pub fn canceled(activity: Activity) -> Self {
        Self { kind: ActivityEventKind::Canceled, activity }
    }
}

// a single line of the activity log that could not be parsed, as reported by `check`
pub struct ParseError<'a> {
    pub line_number: usize,
    pub raw: &'a str,
    pub message: String,
}

// an activity `sanity` considers unusual
pub struct SanityFinding<'a> {
    pub line_number: usize,
    pub activity: &'a Activity,
    pub negative_duration: bool,
    pub overlaps_previous: bool,
}

// a sink for everything a subcommand wants to show the user
//
// every subcommand that produces output goes through one of these, so adding an
// output format means adding one implementation rather than branching inside
// each controller.
pub trait OutputWriter {
    // tracked activities, optionally grouped by start date
    fn activities(
        &self,
        activities: &[&Activity],
        grouped: bool,
        with_start_dates: bool,
    ) -> Result<()>;

    // activities that have not been stopped yet
    fn running_activities(&self, activities: &[&Activity]) -> Result<()>;

    // (index, description, project) triples, as used by `last` and `search`
    fn indexed_activities(
        &self,
        items: &[(usize, &(&String, &String))],
        empty_message: &str,
    ) -> Result<()>;

    // distinct project names
    fn projects(&self, projects: &[&String], no_quotes: bool) -> Result<()>;

    // durations grouped by project and description
    fn report(&self, activities: &[&Activity]) -> Result<()>;

    // the `status` summary
    fn status(&self, data: &StatusReportData) -> Result<()>;

    // events produced by the manipulation subcommands; a single invocation may
    // produce several (e.g. `start` stops running activities before starting one)
    fn activity_events(&self, events: &[ActivityEvent]) -> Result<()>;

    // unparseable lines reported by `check` (empty slice means the file is clean)
    fn parse_errors(&self, errors: &[ParseError]) -> Result<()>;

    // unusual activities reported by `sanity` (empty slice means nothing unusual)
    fn sanity_findings(&self, findings: &[SanityFinding]) -> Result<()>;
}

// the human readable output: tables, colors and terminal aware wrapping
pub struct PlaintextWriter {}

impl OutputWriter for PlaintextWriter {
    fn activities(
        &self,
        activities: &[&Activity],
        grouped: bool,
        with_start_dates: bool,
    ) -> Result<()> {
        if grouped {
            list::list_activities_grouped_by_date(activities);
        } else {
            list::list_activities(activities, with_start_dates);
        }
        Ok(())
    }

    fn running_activities(&self, activities: &[&Activity]) -> Result<()> {
        list::list_running_activities(activities);
        Ok(())
    }

    fn indexed_activities(
        &self,
        items: &[(usize, &(&String, &String))],
        empty_message: &str,
    ) -> Result<()> {
        list::list_descriptions_and_projects_with_index(items, empty_message);
        Ok(())
    }

    fn projects(&self, projects: &[&String], no_quotes: bool) -> Result<()> {
        for project in projects {
            if no_quotes {
                println!("{project}");
            } else {
                println!("\"{project}\"");
            }
        }
        Ok(())
    }

    fn report(&self, activities: &[&Activity]) -> Result<()> {
        report::show_activities(activities);
        Ok(())
    }

    fn status(&self, data: &StatusReportData) -> Result<()> {
        println!("{data}");
        Ok(())
    }

    fn activity_events(&self, events: &[ActivityEvent]) -> Result<()> {
        for event in events {
            let a = &event.activity;
            let start = a.start.format(conf::FORMAT_DATETIME);
            match event.kind {
                ActivityEventKind::Started => println!(
                    "Started activity: \"{}\" ({}) at {}",
                    a.description, a.project, start
                ),
                ActivityEventKind::Changed => println!(
                    "Changed activity: \"{}\" ({}) started at {}",
                    a.description, a.project, start
                ),
                ActivityEventKind::Canceled => println!(
                    "Canceled activity: \"{}\" ({}) started at {}",
                    a.description, a.project, start
                ),
                ActivityEventKind::Stopped => println!(
                    "Stopped activity: \"{}\" ({}) started at {} ({})",
                    a.description,
                    a.project,
                    start,
                    format_util::format_duration(&a.get_duration()),
                ),
            }
        }
        Ok(())
    }

    fn parse_errors(&self, errors: &[ParseError]) -> Result<()> {
        if errors.is_empty() {
            println!("All lines in the file have been successfully parsed as activities.");
            return Ok(());
        }

        println!("Found {} line(s) with parsing errors", errors.len());
        for error in errors {
            println!("\n{}\n  -> {} (Line: {})", error.raw, error.message, error.line_number);
        }
        Ok(())
    }

    fn sanity_findings(&self, findings: &[SanityFinding]) -> Result<()> {
        if findings.is_empty() {
            println!("No unusual activities.");
            return Ok(());
        }

        for finding in findings {
            if finding.negative_duration {
                println!("Activity has negative duration");
            }
            if finding.overlaps_previous {
                println!("Activity started before another activity ended");
            }

            let a = finding.activity;
            println!(
                "{} (Started: {}, Ended: {}, Line: {})\n",
                a.description,
                a.start.format(conf::FORMAT_DATETIME),
                a.end.map_or_else(
                    || String::from("--"),
                    |end| end.format(conf::FORMAT_DATETIME).to_string()
                ),
                finding.line_number
            );
        }
        Ok(())
    }
}
