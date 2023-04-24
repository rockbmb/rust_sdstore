use std::{hash::Hash, str::FromStr, fmt::Display};

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

impl Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Filter::Nop => write!(f, "nop"),
            Filter::Bcompress => write!(f, "bcompress"),
            Filter::Bdecompress => write!(f, "bdecompress"),
            Filter::Gcompress => write!(f, "gcompress"),
            Filter::Gdecompress => write!(f, "gdecompress"),
            Filter::Encrypt => write!(f, "encrypt"),
            Filter::Decrypt => write!(f, "decrypt"),
        }
    }
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