use rand::{self, Rng};
use rocket::request::FromParam;
use std::borrow::Cow;
use std::fmt;

pub const BASE62: &'static [u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const SIZE: usize = 24;

/// A _probably_ unique paste ID.
#[derive(Default)]
pub struct PasteID<'a>(Cow<'a, str>);

impl<'a> PasteID<'a> {
    fn new_with_size(size: usize) -> PasteID<'static> {
        let mut id = String::with_capacity(size);
        let mut rng = rand::thread_rng();
        for _ in 0..size {
            id.push(BASE62[rng.gen::<usize>() % 62] as char);
        }

        PasteID(Cow::Owned(id))
    }

    /// Generate a _probably_ unique ID with `size` characters.
    pub fn new() -> PasteID<'static> {
        PasteID::new_with_size(SIZE)
    }

    pub fn id(self) -> String {
        self.0.into_owned()
    }
}

impl<'a> fmt::Display for PasteID<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}


/// Returns `true` if `id` is a valid paste ID and `false` otherwise.
fn valid_id(id: &str) -> bool {
    id.len() == SIZE &&
    id.chars().all(|c| (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'))
}

/// Returns an instance of `PasteID` if the path segment is a valid ID.
/// Otherwise returns the invalid ID as the `Err` value.
impl<'a> FromParam<'a> for PasteID<'a> {
    type Error = &'a str;
    // TODO make more strict
    fn from_param(param: &'a str) -> Result<PasteID<'a>, &'a str> {
        if valid_id(param) {
            Ok(PasteID(Cow::Borrowed(param)))
        } else {
            Err(param)
        }
    }
}
