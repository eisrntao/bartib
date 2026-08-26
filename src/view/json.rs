use anyhow::Result;
use chrono::NaiveDateTime;
use serde::Serialize;

use crate::data::activity::Activity;
use crate::data::processor::StatusReportData;
use crate::view::output::OutputWriter;
use crate::view::report;

// ISO 8601 without an offset.
//
// Bartib's log deliberately stores local wall clock time with no timezone, so
// claiming an offset here would invent information we do not have (and would be
// wrong for entries recorded on the other side of a DST change). Consumers get
// a sortable, unambiguous string and can attach their own zone if they need one.
static FORMAT_ISO8601: &str = "%Y-%m-%dT%H:%M:%S";

fn format_timestamp(t: NaiveDateTime) -> String {
    t.format(FORMAT_ISO8601).to_string()
}

// Durations are emitted as whole seconds rather than a display string such as
// "1h 30m" or "<1m": the latter cannot be parsed back into a number at all.
#[derive(Serialize)]
struct ActivityJson<'a> {
    start: String,
    end: Option<String>,
    project: &'a str,
    description: &'a str,
    duration_seconds: i64,
    is_running: bool,
}

impl<'a> From<&'a Activity> for ActivityJson<'a> {
    fn from(activity: &'a Activity) -> ActivityJson<'a> {
        ActivityJson {
            start: format_timestamp(activity.start),
            end: activity.end.map(format_timestamp),
            project: &activity.project,
            description: &activity.description,
            duration_seconds: activity.get_duration().num_seconds(),
            is_running: !activity.is_stopped(),
        }
    }
}

#[derive(Serialize)]
struct IndexedActivityJson<'a> {
    index: usize,
    description: &'a str,
    project: &'a str,
}

#[derive(Serialize)]
struct ReportDescriptionJson<'a> {
    description: &'a str,
    duration_seconds: i64,
}

#[derive(Serialize)]
struct ReportProjectJson<'a> {
    project: &'a str,
    duration_seconds: i64,
    descriptions: Vec<ReportDescriptionJson<'a>>,
}

#[derive(Serialize)]
struct ReportJson<'a> {
    projects: Vec<ReportProjectJson<'a>>,
    total_duration_seconds: i64,
}

#[derive(Serialize)]
struct StatusJson<'a> {
    activity: Option<ActivityJson<'a>>,
    project: Option<&'a str>,
    today_seconds: i64,
    current_week_seconds: i64,
    current_month_seconds: i64,
}

// machine readable output: one JSON document per invocation, on stdout
pub struct JsonWriter {}

impl JsonWriter {
    fn print<T: Serialize>(value: &T) -> Result<()> {
        println!("{}", serde_json::to_string(value)?);
        Ok(())
    }
}

impl OutputWriter for JsonWriter {
    // `grouped` and `with_start_dates` only affect terminal layout, so JSON
    // ignores them: the full timestamp is always present on every entry.
    fn activities(
        &self,
        activities: &[&Activity],
        _grouped: bool,
        _with_start_dates: bool,
    ) -> Result<()> {
        let out: Vec<ActivityJson> = activities.iter().map(|a| (*a).into()).collect();
        Self::print(&out)
    }

    fn running_activities(&self, activities: &[&Activity]) -> Result<()> {
        let out: Vec<ActivityJson> = activities.iter().map(|a| (*a).into()).collect();
        Self::print(&out)
    }

    fn indexed_activities(
        &self,
        items: &[(usize, &(&String, &String))],
        _empty_message: &str,
    ) -> Result<()> {
        let out: Vec<IndexedActivityJson> = items
            .iter()
            .map(|(index, (description, project))| IndexedActivityJson {
                index: *index,
                description,
                project,
            })
            .collect();
        Self::print(&out)
    }

    // `no_quotes` exists so shell scripts can consume the plaintext list; JSON
    // quoting is decided by the encoder.
    fn projects(&self, projects: &[&String], _no_quotes: bool) -> Result<()> {
        Self::print(&projects)
    }

    fn report(&self, activities: &[&Activity]) -> Result<()> {
        let project_map = report::create_project_map(activities);

        let projects = project_map
            .iter()
            .map(|(project, (activities, duration))| {
                let descriptions = report::group_activities_by_description(activities)
                    .iter()
                    .map(|(description, activities)| ReportDescriptionJson {
                        description,
                        duration_seconds: report::sum_duration(activities).num_seconds(),
                    })
                    .collect();

                ReportProjectJson {
                    project,
                    duration_seconds: duration.num_seconds(),
                    descriptions,
                }
            })
            .collect();

        Self::print(&ReportJson {
            projects,
            total_duration_seconds: report::sum_duration(activities).num_seconds(),
        })
    }

    fn status(&self, data: &StatusReportData) -> Result<()> {
        Self::print(&StatusJson {
            activity: data.activity.map(Into::into),
            project: data.project,
            today_seconds: data.today.num_seconds(),
            current_week_seconds: data.current_week.num_seconds(),
            current_month_seconds: data.current_month.num_seconds(),
        })
    }
}
