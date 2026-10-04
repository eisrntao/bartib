use anyhow::{anyhow, bail, Context, Result};
use chrono::NaiveDateTime;
use std::process::Command;

use crate::data::activity;
use crate::data::bartib_file;
use crate::data::getter;
use crate::view::output::{ActivityEvent, OutputWriter};

// starts a new activity
pub fn start(
    file_name: &str,
    project_name: &str,
    activity_description: &str,
    time: Option<NaiveDateTime>,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let mut file_content: Vec<bartib_file::Line> = Vec::new();
    let mut events: Vec<ActivityEvent> = Vec::new();

    if let Ok(mut previous_file_content) = bartib_file::get_file_content(file_name) {
        // if we start a new activities programaticly, we stop all other activities first.
        // However, we must not assume that there is always only one activity
        // running as the user may have started activities manually
        for stopped in stop_all_running_activities(&mut previous_file_content, time) {
            events.push(ActivityEvent::stopped(stopped));
        }

        file_content.append(&mut previous_file_content);
    }

    let activity = activity::Activity::start(
        project_name.to_string(),
        activity_description.to_string(),
        time,
    );

    save_new_activity(file_name, &mut file_content, activity, &mut events)?;
    writer.activity_events(&events)
}

fn save_new_activity(
    file_name: &str,
    file_content: &mut Vec<bartib_file::Line>,
    activity: activity::Activity,
    events: &mut Vec<ActivityEvent>,
) -> Result<()> {
    events.push(ActivityEvent::started(activity.clone()));

    file_content.push(bartib_file::Line::for_activity(activity));
    bartib_file::write_to_file(file_name, file_content)
        .context(format!("Could not write to file: {file_name}"))
}

pub fn change(
    file_name: &str,
    project_name: Option<&str>,
    activity_description: Option<&str>,
    time: Option<NaiveDateTime>,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let mut file_content = bartib_file::get_file_content(file_name)?;
    let mut events: Vec<ActivityEvent> = Vec::new();

    for line in &mut file_content {
        if let Ok(activity) = &mut line.activity {
            if !activity.is_stopped() {
                let mut changed = false;

                if let Some(project_name) = project_name {
                    activity.project = project_name.to_string();
                    changed = true;
                }

                if let Some(activity_description) = activity_description {
                    activity.description = activity_description.to_string();
                    changed = true;
                }

                if let Some(time) = time {
                    activity.start = time;
                    changed = true;
                }

                if changed {
                    events.push(ActivityEvent::changed(activity.clone()));
                    line.set_changed();
                }
            }
        }
    }
    bartib_file::write_to_file(file_name, &file_content)
        .context(format!("Could not write to file: {file_name}"))?;
    writer.activity_events(&events)
}

// stops all currently running activities
pub fn stop(
    file_name: &str,
    time: Option<NaiveDateTime>,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let mut file_content = bartib_file::get_file_content(file_name)?;
    let stopped = stop_all_running_activities(&mut file_content, time);
    bartib_file::write_to_file(file_name, &file_content)
        .context(format!("Could not write to file: {file_name}"))?;
    let events: Vec<ActivityEvent> = stopped.into_iter().map(ActivityEvent::stopped).collect();
    writer.activity_events(&events)
}

// cancels all currently running activities
pub fn cancel(file_name: &str, writer: &dyn OutputWriter) -> Result<()> {
    let file_content = bartib_file::get_file_content(file_name)?;
    let mut new_file_content: Vec<bartib_file::Line> = Vec::new();
    let mut events: Vec<ActivityEvent> = Vec::new();

    for line in file_content {
        match &line.activity {
            Ok(activity) => {
                if activity.is_stopped() {
                    new_file_content.push(line);
                } else {
                    events.push(ActivityEvent::canceled(activity.clone()));
                }
            }
            Err(_) => new_file_content.push(line),
        }
    }

    bartib_file::write_to_file(file_name, &new_file_content)
        .context(format!("Could not write to file: {file_name}"))?;
    writer.activity_events(&events)
}

// continue last activity
pub fn continue_last_activity(
    file_name: &str,
    project_name: Option<&str>,
    activity_description: Option<&str>,
    time: Option<NaiveDateTime>,
    number: usize,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let mut file_content = bartib_file::get_file_content(file_name)?;

    let descriptions_and_projects: Vec<(&String, &String)> =
        getter::get_descriptions_and_projects(&file_content);

    if descriptions_and_projects.is_empty() {
        bail!("No activity has been started before.")
    }

    // the activities are addressed by a zero based index, so the highest valid
    // number is one less than the count
    if number >= descriptions_and_projects.len() {
        bail!(format!(
            "Only {} distinct activities have been logged yet, so the highest number to continue is {}",
            descriptions_and_projects.len(),
            descriptions_and_projects.len() - 1
        ));
    }

    // the list runs oldest to newest while the number counts back from the
    // newest, and the check above guarantees this stays in bounds
    let (description, project) =
        descriptions_and_projects[descriptions_and_projects.len() - number - 1];

    let new_activity = activity::Activity::start(
        project_name.unwrap_or(project).to_string(),
        activity_description.unwrap_or(description).to_string(),
        time,
    );

    let mut events: Vec<ActivityEvent> = stop_all_running_activities(&mut file_content, time)
        .into_iter()
        .map(ActivityEvent::stopped)
        .collect();
    save_new_activity(file_name, &mut file_content, new_activity, &mut events)?;
    writer.activity_events(&events)
}

pub fn toggle(
    file_name: &str,
    project_name: Option<&str>,
    activity_description: Option<&str>,
    time: Option<NaiveDateTime>,
    writer: &dyn OutputWriter,
) -> Result<()> {
    let mut file_content = bartib_file::get_file_content(file_name)?;
    if file_content.is_empty() {
        bail!("No activity has been started before.")
    }

    let stopped = stop_all_running_activities(&mut file_content, time);
    if stopped.is_empty() {
        continue_last_activity(file_name, project_name, activity_description, time, 0, writer)
    } else {
        bartib_file::write_to_file(file_name, &file_content)
            .context(format!("Could not write to file: {file_name}"))?;
        let events: Vec<ActivityEvent> =
            stopped.into_iter().map(ActivityEvent::stopped).collect();
        writer.activity_events(&events)
    }
}

pub fn start_editor(file_name: &str, optional_editor_command: Option<&str>) -> Result<()> {
    let editor_command = optional_editor_command.context("editor command is missing")?;
    let command = Command::new(editor_command).arg(file_name).spawn();

    match command {
        Ok(mut child) => {
            child.wait().context("editor did not execute")?;
            Ok(())
        }
        Err(e) => Err(anyhow!(e)),
    }
}

// stops every running activity in place and returns a snapshot of each one that
// was stopped, so callers can report them in whatever output format is active
fn stop_all_running_activities(
    file_content: &mut [bartib_file::Line],
    time: Option<NaiveDateTime>,
) -> Vec<activity::Activity> {
    let mut stopped = Vec::new();

    for line in file_content {
        if let Ok(activity) = &mut line.activity {
            if !activity.is_stopped() {
                activity.stop(time);
                stopped.push(activity.clone());
                line.set_changed();
            }
        }
    }

    stopped
}
