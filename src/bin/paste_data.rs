use rocket::Outcome;
use rocket::Request;
use rocket::data::{self, FromData, Data};
use rocket::http::{Status, ContentType};
use std::io::{Read};

#[derive(Serialize, Deserialize)]
pub struct PasteData {
    content: String,
}

impl PasteData {
    pub fn get_content_cloned(&self) -> String {
        self.content.clone()
    }
}

impl FromData for PasteData {
    type Error = String;

    fn from_data(req: &Request, data: Data) -> data::Outcome<Self, String> {
        let corr_content_type = ContentType::new("text", "plain");
        if req.content_type().expect("Could not extract content type") != corr_content_type {
            return Outcome::Forward(data);
        }

        // Check size //TODO which size?
        let max_size = 4 * 1024 * 1024 + 6; // +6 because we have "paste=" in it
        let req_headers = req.headers();
        let content_len_it = req_headers.get("Content-Length");
        for c in content_len_it {
            let content_len = c.parse::<u64>().unwrap();
            if content_len > max_size {
                return Outcome::Failure((Status::PayloadTooLarge, "Content too big!".into()));
            }
        }

        // Read data
        let mut data_string = String::new();
        if let Err(e) = data.open().read_to_string(&mut data_string) {
            return Outcome::Failure((Status::InternalServerError, format!("{:?}", e)));
        }
        // remove the "paste=" from the raw data //TODO Problem: paste= must be at end of request
        let real_data = match data_string.find("paste=") {
            Some(i) => &data_string[(i + 6)..],
            None => return Outcome::Failure((Status::BadRequest, "Missing paste parameter.".into())),
        };
        Outcome::Success(PasteData { content: real_data.to_string() })
    }
}
