use std::{fs, io};

pub mod util;

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    nop: usize,
    bcompress: usize,
    bdecompress: usize,
    gcompress: usize,
    gdecompress: usize,
    encrypt: usize,
    decrypt: usize
}

#[derive(Debug)]
pub enum ConfigParseError {
    LineParseError,
    FilterLimitParseError(String),
    NoConfigFileProvided,
    ConfigFileReadError(io::Error)
}

impl Config {
    pub fn default() -> Config {
        Config {
            nop: 0,
            bcompress: 0,
            bdecompress: 0,
            gcompress: 0,
            gdecompress: 0,
            encrypt: 0,
            decrypt: 0
        }
    }

    pub fn parse(s: &str) -> Result<Config, ConfigParseError> {
        let mut conf = Self::default();

        for l in s.lines() {
            let mut words = l.split_whitespace();
            let opt_filter = words.next();
            let opt_count = words.next();

            let (filter, count) = match (opt_filter, opt_count) {
                (_, None) | (None, _) => return Err(ConfigParseError::LineParseError),
                (Some(filter), Some(count)) => {
                    let count: usize = match count.trim().parse() {
                        Err(_) => return Err(ConfigParseError::FilterLimitParseError(filter.to_string())),
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

    pub fn build(mut args: impl Iterator<Item = String>) -> Result<Config, ConfigParseError> {
        // Move past executable name in args list
        args.next();

        let file_path = match args.next() {
            Some(arg) => arg,
            None => return Err(ConfigParseError::NoConfigFileProvided),
        };

        let file = match fs::read_to_string(file_path) {
            Err(io_err) => return Err(ConfigParseError::ConfigFileReadError(io_err)),
            Ok(fd) => fd,
        };

        Config::parse(&file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing_works() {
        let expected_config = Config {
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

        let read_config = Config::parse(config_txt).expect("parsing should succeed");
        assert_eq!(expected_config, read_config);
    }

    #[test]
    fn config_parsing_fails1() {
        let config_txt = "nop 3cccc";

        assert!(
            matches!(
                Config::parse(config_txt).unwrap_err(),
                ConfigParseError::FilterLimitParseError(_)
            )
        )
    }

    #[test]
    fn config_parsing_fails2() {
        let config_txt = "nop7";

        assert!(matches!(Config::parse(config_txt).unwrap_err(), ConfigParseError::LineParseError))
    }
}
