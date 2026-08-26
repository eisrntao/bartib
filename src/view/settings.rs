use clap::ArgMatches;

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
}
