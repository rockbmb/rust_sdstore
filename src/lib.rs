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

#[derive(Debug, PartialEq)]
pub enum ConfigParseError {
    LineParseError,
    FilterLimitParseError(String)
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
        let parse_err = Err(ConfigParseError::FilterLimitParseError("nop".to_string()));

        assert_eq!(Config::parse(config_txt), parse_err)
    }

    #[test]
    fn config_parsing_fails2() {
        let config_txt = "nop7";

        let parse_err = ConfigParseError::LineParseError;
        assert_eq!(Config::parse(config_txt), Err(parse_err))
    }
}
