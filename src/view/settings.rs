use clap::ArgMatches;

use crate::view::output::{OutputWriter, PlaintextWriter};

#[cfg(feature = "json")]
use crate::view::json::JsonWriter;

#[cfg(feature = "json")]
pub enum OutputFormat {
    Plaintext,
    Json,
}

pub struct CliSettings {
    #[cfg(feature = "json")]
    pub output_format: OutputFormat,
}

impl CliSettings {
    pub fn from_matches(matches: &ArgMatches) -> CliSettings {
        #[cfg(not(feature = "json"))]
        let _ = matches;

        CliSettings {
            #[cfg(feature = "json")]
            output_format: if matches.is_present("json") {
                OutputFormat::Json
            } else {
                OutputFormat::Plaintext
            },
        }
    }

    // the one place the output format is turned into behaviour
    #[must_use]
    pub fn create_writer(&self) -> Box<dyn OutputWriter> {
        #[cfg(feature = "json")]
        if let OutputFormat::Json = self.output_format {
            return Box::new(JsonWriter {});
        }

        Box::new(PlaintextWriter {})
    }
}
