use std::io;
use std::path::Path;
use rocket::Request;
use rocket::data::{self, FromData, Data};
use rocket::Outcome;
use rocket::http::{Status, ContentType};

pub struct PasteData {
    content: Data,
}

impl PasteData {
    pub fn stream_to_file<P: AsRef<Path>>(self, path: P) -> Result<u64, io::Error> {
        self.content.stream_to_file(path)
    }
}

impl FromData for PasteData {
    type Error = String;

    fn from_data(req: &Request, data: Data) -> data::Outcome<Self, String> {
        let corr_content_type = ContentType::new("text", "plain");
        if req.content_type() != corr_content_type {
            return Outcome::Forward(data);
        }

        let max_size = 4 * 1024 * 1024; //TODO
        let req_headers = req.headers();
        let content_len_it = req_headers.get("Content-Length");
        for c in content_len_it {
            let content_len = c.parse::<u64>().unwrap();
            if content_len > max_size {
                return Outcome::Failure((Status::PayloadTooLarge, "Content too big!".into()));
            }
        }
        Outcome::Success(PasteData { content: data })
    }
}
