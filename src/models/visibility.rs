use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
pub enum Visibility {
    Active,
    Published,
}

impl FromStr for Visibility {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_ref() {
            "active" => Ok(Visibility::Active),
            "published" => Ok(Visibility::Published),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Visibility::Active => "active",
            Visibility::Published => "published",
        };
        write!(f, "{}", s)
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Published
    }
}
