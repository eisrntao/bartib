use clap::ArgMatches;

pub enum OutputFormat {
    Plaintext,
    Json,
}
pub struct CliSettings {
    pub output_format: OutputFormat,
    pub nowarn: bool,
}

impl CliSettings {
    pub fn from_matches(matches: &ArgMatches) -> CliSettings {
        let output_format = if matches.is_present("json") {
            OutputFormat::Json
        } else {
            OutputFormat::Plaintext
        };
        CliSettings {
            output_format,
            nowarn: matches.is_present("nowarn"),
        }
    }
}
