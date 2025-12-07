use anyhow::Result;
use chrono::Duration;

#[cfg(feature = "json")]
use crate::view::format_util::format_duration;
#[cfg(feature = "json")]
use serde::ser::SerializeStruct;
#[cfg(feature = "json")]
use serde::{Serialize, Serializer};

use crate::data::activity;
use crate::data::round_util::round_datetime;

pub type ProcessorList = Vec<Box<dyn ActivityProcessor>>;

pub trait ActivityProcessor {
    fn process(&self, activity: &activity::Activity) -> activity::Activity;
}

pub struct StatusReportData<'a> {
    pub activity: Option<&'a activity::Activity>,
    pub project: Option<&'a str>,
    pub today: Duration,
    pub current_week: Duration,
    pub current_month: Duration,
}

#[cfg(feature = "json")]
impl<'a> Serialize for StatusReportData<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("StatusReportData", 4)?;
        s.serialize_field("activity", &self.activity)?;
        s.serialize_field("project", &self.project)?;
        s.serialize_field("today", &format_duration(&self.today))?;
        s.serialize_field("current_week", &format_duration(&self.current_week))?;
        s.serialize_field("current_month", &format_duration(&self.current_month))?;
        s.end()
    }
}
pub trait StatusReportWriter {
    fn process(&self, data: &StatusReportData) -> Result<()>;
}

pub struct RoundProcessor {
    pub round: Duration,
}

impl ActivityProcessor for RoundProcessor {
    fn process(&self, activity: &activity::Activity) -> activity::Activity {
        let start = round_datetime(&activity.start, &self.round);
        let end = activity.end.map(|end| round_datetime(&end, &self.round));

        activity::Activity {
            start,
            end,
            project: activity.project.clone(),
            description: activity.description.clone(),
        }
    }
}

pub fn process_activities(
    activities: Vec<&activity::Activity>,
    processors: ProcessorList,
) -> Vec<activity::Activity> {
    activities
        .into_iter()
        .cloned()
        .map(|activity| {
            processors
                .iter()
                .fold(activity, |activity, processor| processor.process(&activity))
        })
        .collect()
}
