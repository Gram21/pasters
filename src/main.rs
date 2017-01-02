#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate rand;
mod paste_id;
mod lang;
mod paste_data;

use paste_id::PasteID;
use lang::PasteLang;
use paste_data::PasteData;
use std::io::{Result, Error};
use std::path::{Path, PathBuf};
use std::fs::{File, remove_file};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use rocket::response::{status, NamedFile};
use rocket::request::Form;
use rocket::http::Status;
use rocket_contrib::Template;

fn main() {
    thread::spawn(|| {
        loop {
            let interval = 60;
            let max_time_alive = Duration::new(60 * 60 * 24 * 7, 0);
            match remove_old_files(max_time_alive) {
                Ok(_) => thread::sleep(Duration::from_secs(interval)),
                Err(err) => {
                    println!("Error: {}", err);
                    thread::sleep(Duration::from_secs(interval))
                }
            }
        }
    });
    rocket::ignite()
        .catch(errors![not_found, too_large])
        .mount("/",
               routes![get_static, index, upload, update, retrieve, remove])
        .launch()
}

fn remove_old_files(max_time_alive: Duration) -> Result<bool> {
    let mut removed = false;
    if let Ok(dir) = Path::new("upload/").read_dir() {
        for dir_entry_wrapped in dir {
            let dir_entry = try!(dir_entry_wrapped);
            let metadata = dir_entry.metadata().unwrap();
            if let Ok(time) = metadata.modified() {
                let time_alive = time.elapsed().unwrap();
                if time_alive > max_time_alive {
                    if remove_file(dir_entry.path()).is_ok() {
                        removed = true;
                        println!("Removed paste with id {}",
                                 dir_entry.file_name().to_str().unwrap());
                    }
                }
            } else {
                return Err(Error::last_os_error());
            }
        }
        Ok(removed)
    } else {
        Err(Error::last_os_error())
    }
}

#[error(404)]
fn not_found(req: &rocket::Request) -> Template {
    let mut map = HashMap::new();
    map.insert("path", req.uri().as_str());
    Template::render("404", &map)
}

#[error(413)]
fn too_large() -> Template {
    let mut map = HashMap::new();
    map.insert("error", "Too large!");
    Template::render("index", &map)
}

#[get("/")]
fn index() -> Template {
    let map: HashMap<&str, &str> = std::collections::HashMap::new();
    Template::render("index", &map)
}

#[post("/", format="text/plain", data = "<paste>")]
fn upload(paste: PasteData) -> Template {
    let id = PasteID::new(24);
    let mut map = HashMap::new();
    match write_to_file(paste, id) {
        Ok(res) => map.insert("success_create", res.1.to_string()),
        Err(res) => map.insert("error", res.to_string()),
    };
    Template::render("index", &map)
}

#[put("/<id>", format="text/plain", data = "<paste>")]
fn update(id: PasteID, paste: PasteData) -> Result<status::Custom<String>> {
    write_to_file(paste, id)
}

fn write_to_file(paste: PasteData, id: PasteID) -> Result<status::Custom<String>> {
    let filename = format!("upload/{id}", id = id);
    let output = format!("{host}/{id}", host = "http://localhost:8000", id = id);

    paste.stream_to_file(Path::new(&filename))?;
    Ok(status::Custom(Status::Created, output))
}

#[get("/<id>", format="text/plain")]
fn retrieve(id: PasteID) -> Option<File> {
    // TODO with template etc
    let filename = format!("upload/{id}", id = id);
    File::open(&filename).ok()
}

#[derive(FromForm)]
struct PasteDel<'r> {
    paste_id: &'r str,
    paste_key: &'r str,
}

#[post("/remove", data = "<del_form>")]
fn remove<'a>(del_form: Form<'a, PasteDel<'a>>) -> Template {
    let paste_del = del_form.get();
    let filename = format!("upload/{id}", id = paste_del.paste_id);
    let file = Path::new(&filename);
    let mut map = HashMap::new();
    if file.exists() {
        let key = paste_del.paste_key;
        if key == "password" {
            if remove_file(file).is_ok() {
                map.insert("success",
                           format!("Paste {id} removed", id = paste_del.paste_id));
            }
        } else {
            map.insert("error", "Invalid Paste ID or Key".into());
        }
    } else {
        map.insert("error", "Invalid Paste ID or Key".into());
    }
    Template::render("index", &map)
}

#[get("/static/<file..>")]
fn get_static(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).ok()
}
