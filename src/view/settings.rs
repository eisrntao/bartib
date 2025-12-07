use clap::ArgMatches;

#[cfg(feature = "json")]
pub enum OutputFormat {
    Plaintext,
    Json,
}
pub struct CliSettings {
    #[cfg(feature = "json")]
    pub output_format: OutputFormat,
    pub nowarn: bool,
}

impl CliSettings {
    pub fn from_matches(matches: &ArgMatches) -> CliSettings {
        #[cfg(feature = "json")]
        let output_format = if matches.is_present("json") {
            OutputFormat::Json
        } else {
            OutputFormat::Plaintext
        };

        #[cfg(not(feature = "json"))]
        return CliSettings {
            nowarn: matches.is_present("nowarn"),
        };
        #[cfg(feature = "json")]
        CliSettings {
            output_format,
            nowarn: matches.is_present("nowarn"),
        }
    }
}
