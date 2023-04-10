use std::{fs, io, path::{Path, PathBuf}};

/// Representation of the maximum allowed concurrent instances of each filter
/// the server is permitted to run.
///
/// This is to be read from a file passed to the server executable.
#[derive(Debug, PartialEq, Eq)]
pub struct FiltersConfig {
    nop: usize,
    bcompress: usize,
    bdecompress: usize,
    gcompress: usize,
    gdecompress: usize,
    encrypt: usize,
    decrypt: usize
}

/// Errors that may happen when parsing a server's filter limits config file.
#[derive(Debug)]
pub enum FilterCfgParseError {
    LineParseError,
    FilterLimitParseError(String),
    NoConfigFileProvided,
    ConfigFileReadError(io::Error)
}

impl FiltersConfig {
    pub fn default() -> Self {
        FiltersConfig {
            nop: 0,
            bcompress: 0,
            bdecompress: 0,
            gcompress: 0,
            gdecompress: 0,
            encrypt: 0,
            decrypt: 0
        }
    }

    /// Parse a `FilterConfig` from a file provided by the user.
    ///
    /// The file must be composed of lines of ASCII, where each line
    /// is of the form:
    ///
    /// ```
    /// <filter-name> <nonnegative-integer>
    /// ```
    pub fn parse(s: &str) -> Result<Self, FilterCfgParseError> {
        let mut conf = Self::default();

        for l in s.lines() {
            let mut words = l.split_whitespace();
            let opt_filter = words.next();
            let opt_count = words.next();
            let (filter, count) = match (opt_filter, opt_count) {
                (_, None) | (None, _) => return Err(FilterCfgParseError::LineParseError),
                (Some(filter), Some(count)) => {
                    let count: usize = match count.trim().parse() {
                        Err(_) => return Err(FilterCfgParseError::FilterLimitParseError(filter.to_string())),
                        Ok(c) => c
                    };
                    (filter, count)
                },
            };
            match filter {
                "nop" => conf.nop = count,
                "bcompress" => conf.bcompress = count,
                "bdecompress" => conf.bdecompress = count,
                "gcompress" => conf.gcompress = count,
                "gdecompress" => conf.gdecompress = count,
                "encrypt" => conf.encrypt = count,
                "decrypt" => conf.decrypt = count,
                _ => {}
            }
        }

        Ok(conf)
    }

    pub fn build(args: &mut impl Iterator<Item = String>) -> Result<Self, FilterCfgParseError> {
        let file_path = match args.next() {
            Some(arg) => arg,
            None => return Err(FilterCfgParseError::NoConfigFileProvided),
        };

        let file = match fs::read_to_string(file_path) {
            Err(io_err) => return Err(FilterCfgParseError::ConfigFileReadError(io_err)),
            Ok(fd) => fd,
        };

        FiltersConfig::parse(&file)
    }
}

/// Full configuration for a server: filters, and path to filter executables.
#[derive(Debug)]
pub struct ServerConfig {
    filters_config: FiltersConfig,
    transformations_path: PathBuf
}

impl ServerConfig {
    pub fn transformations_path(&self) -> &Path {
        self.transformations_path.as_path()
    }
}

#[derive(Debug)]
pub enum ServerCfgParseError {
    NoTransformationsPathGiven,
    FilterCfgParseError(FilterCfgParseError)
}

impl ServerConfig {
    pub fn build(args: &mut impl Iterator<Item = String>) -> Result<Self, ServerCfgParseError> {
        // Move past executable name in args list
        args.next();

        let filters_config = match FiltersConfig::build(args) {
            Err(err) => return Err(ServerCfgParseError::FilterCfgParseError(err)),
            Ok(f) => f,
        };

        let transformations_path = match args.next() {
            None => return Err(ServerCfgParseError::NoTransformationsPathGiven),
            Some(s) => PathBuf::from(s),
        };

        Ok(ServerConfig { filters_config, transformations_path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_works() {
        let expected_config = FiltersConfig {
            nop: 3,
            bcompress: 4,
            bdecompress: 4,
            gcompress: 2,
            gdecompress: 2,
            encrypt: 2,
            decrypt: 2
        };

        let config_txt = "nop 3
        bcompress 4
        bdecompress 4
        gcompress 2
        gdecompress 2
        encrypt 2
        decrypt 2";

        let read_config = FiltersConfig::parse(config_txt).expect("parsing should succeed");
        assert_eq!(expected_config, read_config);
    }

    #[test]
    fn config_parsing_fails1() {
        let config_txt = "nop 3cccc";

        assert!(
            matches!(
                FiltersConfig::parse(config_txt).unwrap_err(),
                FilterCfgParseError::FilterLimitParseError(_)
            )
        )
    }

    #[test]
    fn config_parsing_fails2() {
        let config_txt = "nop7";

        assert!(matches!(FiltersConfig::parse(config_txt).unwrap_err(), FilterCfgParseError::LineParseError))
    }
}
