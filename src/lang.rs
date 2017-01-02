use std::fmt;
use std::borrow::Cow;
use rocket::request::FromParam;

pub struct PasteLang<'a>(Cow<'a, str>);

impl<'a> fmt::Display for PasteLang<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn valid_lang(lang: &str) -> bool {
    match lang {
        "Java" => true,
        "Rust" => true,
        _ => false,
    }
}

impl<'a> FromParam<'a> for PasteLang<'a> {
    type Error = &'a str;

    fn from_param(param: &'a str) -> Result<PasteLang<'a>, &'a str> {
        match valid_lang(param) {
            true => Ok(PasteLang(Cow::Borrowed(param))),
            false => Err(param),
        }
    }
}
