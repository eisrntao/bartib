use anyhow::Result;
use chrono::NaiveDateTime;
use wildmatch::WildMatch;

use crate::data::activity;
use crate::data::activity::Activity;
use crate::data::bartib_file;
use crate::data::getter;
use crate::data::processor;
use crate::view::output::{OutputWriter, ParseError, SanityFinding};

// lists all currently running activities.
pub fn list_running(file_name: &str, writer: &dyn OutputWriter) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;
    let running_activities = getter::get_running_activities(&file_content);

    writer.running_activities(&running_activities)
}

// lists tracked activities
//
// the activities will be ordered chronologically.
pub fn list(
    file_name: &str,
    filter: getter::ActivityFilter,
    do_group_activities: bool,
    processors: processor::ProcessorList,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;
    let activities = getter::get_activities(&file_content).collect();
    let processed_activities_bind: Vec<activity::Activity> =
        processor::process_activities(activities, processors);
    let processed_activities: Vec<&activity::Activity> = processed_activities_bind.iter().collect();

    let mut filtered_activities: Vec<&activity::Activity> =
        getter::filter_activities(processed_activities, &filter);

    filtered_activities.sort_by_key(|activity| activity.start);

    let first_element = filtered_activities.len().saturating_sub(
        filter
            .number_of_activities
            .unwrap_or(filtered_activities.len()),
    );

    writer.activities(
        &filtered_activities[first_element..],
        do_group_activities,
        filter.date.is_none(),
    )
}

// checks the file content for sanity
pub fn sanity_check(file_name: &str, writer: &dyn OutputWriter) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;
    let mut lines_with_activities: Vec<(Option<usize>, Activity)> = file_content
        .into_iter()
        .filter_map(|line| match line.activity {
            Ok(a) => Some((line.line_number, a)),
            Err(_) => None,
        })
        .collect();
    lines_with_activities.sort_unstable_by_key(|(_, activity)| activity.start);

    let mut last_end: Option<NaiveDateTime> = None;
    let mut findings: Vec<(usize, Activity, bool, bool)> = Vec::new();

    for (line_number, activity) in lines_with_activities {
        let negative_duration = activity.get_signed_duration().num_milliseconds() < 0;
        let overlaps_previous = last_end.is_some_and(|e| e > activity.start);

        if negative_duration || overlaps_previous {
            findings.push((
                line_number.unwrap_or(0),
                activity.clone(),
                negative_duration,
                overlaps_previous,
            ));
        }

        if let Some(e) = last_end {
            if let Some(this_end) = activity.end {
                if this_end > e {
                    last_end = Some(this_end);
                }
            }
        } else {
            last_end = activity.end;
        }
    }

    let findings: Vec<SanityFinding> = findings
        .iter()
        .map(|(line_number, activity, negative_duration, overlaps_previous)| SanityFinding {
            line_number: *line_number,
            activity,
            negative_duration: *negative_duration,
            overlaps_previous: *overlaps_previous,
        })
        .collect();

    writer.sanity_findings(&findings)
}

// reports all errors that occurred when reading the bartib file
pub fn check(file_name: &str, writer: &dyn OutputWriter) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;

    let errors: Vec<ParseError> = file_content
        .iter()
        .filter_map(|line| match &line.activity {
            Err(e) => line.plaintext.as_ref().map(|raw| ParseError {
                line_number: line.line_number.unwrap_or(0),
                raw,
                message: e.to_string(),
            }),
            Ok(_) => None,
        })
        .collect();

    writer.parse_errors(&errors)
}

// lists all projects
pub fn list_projects(
    file_name: &str,
    current: bool,
    no_quotes: bool,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;

    let mut all_projects: Vec<&String> = getter::get_activities(&file_content)
        .filter(|activity| !(current && activity.is_stopped()))
        .map(|activity| &activity.project)
        .collect();

    all_projects.sort_unstable();
    all_projects.dedup();

    writer.projects(&all_projects, no_quotes)
}

// return last finished activity
pub fn list_last_activities(
    file_name: &str,
    number: usize,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;

    let descriptions_and_projects: Vec<(&String, &String)> =
        getter::get_descriptions_and_projects(&file_content);
    let first_element = descriptions_and_projects.len().saturating_sub(number);

    let indexed: Vec<(usize, &(&String, &String))> = descriptions_and_projects[first_element..]
        .iter()
        .rev()
        .enumerate()
        .rev()
        .collect();

    writer.indexed_activities(&indexed, "No activities have been tracked yet")
}

// searches for the term in descriptions and projects
pub fn search(file_name: &str, search_term: Option<&str>, writer: &dyn OutputWriter) -> Result<()> {
    let search_term = search_term
        .map(|term| format!("*{}*", term.to_lowercase()))
        .unwrap_or("".to_string());
    let file_content = bartib_file::get_file_content(file_name)?;

    let descriptions_and_projects: Vec<(&String, &String)> =
        getter::get_descriptions_and_projects(&file_content);
    let search_term_wildmatch = WildMatch::new(&search_term);
    let matches: Vec<(usize, &(&String, &String))> = descriptions_and_projects
        .iter()
        .rev()
        .enumerate()
        .rev()
        .filter(|(_index, (desc, proj))| {
            search_term_wildmatch.matches(&desc.to_lowercase())
                || search_term_wildmatch.matches(&proj.to_lowercase())
        })
        .collect();

    writer.indexed_activities(&matches, "No matching activities found")
}
