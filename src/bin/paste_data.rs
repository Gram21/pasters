use std::io;
use std::io::{Write, BufWriter, Read};
use std::path::Path;
use std::fs::File;
use rocket::Request;
use rocket::data::{self, FromData, Data};
use rocket::Outcome;
use rocket::http::{Status, ContentType};

pub struct PasteData {
    content: String,
}

impl PasteData {
    pub fn stream_to_file<P: AsRef<Path>>(self, path: P) -> Result<(), io::Error> {
        let f = File::create(path).expect("Unable to create file");
        let mut f = BufWriter::new(f);
        f.write_all(self.content.as_bytes())
    }
}

impl FromData for PasteData {
    type Error = String;

    fn from_data(req: &Request, data: Data) -> data::Outcome<Self, String> {
        let corr_content_type = ContentType::new("text", "plain");
        if req.content_type() != corr_content_type {
            return Outcome::Forward(data);
        }

        // Check size
        let max_size = 4 * 1024 * 1024; //TODO
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
        // remove the "paste=" from the raw data
        let real_data = match data_string.find('=') {
            Some(i) => &data_string[(i + 1)..],
            None => return Outcome::Failure((Status::BadRequest, "Missing 'paste='.".into())),
        };

        Outcome::Success(PasteData { content: real_data.to_string() })
    }
}
