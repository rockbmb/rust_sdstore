use std::{hash::Hash, str::FromStr};

use serde::{Serialize, Deserialize};

/// Enum representing the kinds of filters a client can request be applied
/// to a file.
///
/// For each of these variants, there will be a corresponding `.c` source and
/// executable in the `bin/` folder, in the root of this project.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Hash)]
pub enum Filter {
    Nop,
    Bcompress,
    Bdecompress,
    Gcompress,
    Gdecompress,
    Encrypt,
    Decrypt
}

/// Enum for errors gotten while parsing each filter from the client's user input.
#[derive(Debug, PartialEq, Eq)]
pub struct FilterParseError(pub String);

impl FromStr for Filter {
    type Err = FilterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = match s.to_lowercase().as_str() {
            "nop"         => Filter::Nop,
            "bcompress"   => Filter::Bcompress,
            "bdecompress" => Filter::Bdecompress,
            "gcompress"   => Filter::Gcompress,
            "gdecompress" => Filter::Gdecompress,
            "encrypt"     => Filter::Encrypt,
            "decrypt"     => Filter::Decrypt,
            s             => return Err(FilterParseError(s.to_string()))
        };

        Ok(res)
    }
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        match self {
            Filter::Nop => String::from("nop"),
            Filter::Bcompress => String::from("bcompress"),
            Filter::Bdecompress => String::from("bdecompress"),
            Filter::Gcompress => String::from("gcompress"),
            Filter::Gdecompress => String::from("gdecompress"),
            Filter::Encrypt => String::from("encrypt"),
            Filter::Decrypt => String::from("decrypt"),
        }
    }
}