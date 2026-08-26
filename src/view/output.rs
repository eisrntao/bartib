use anyhow::Result;

use crate::data::activity::Activity;
use crate::data::processor::StatusReportData;
use crate::view::list;
use crate::view::report;

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
}
