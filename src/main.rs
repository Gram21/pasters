#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]

extern crate rocket;
extern crate rocket_contrib;
extern crate rand;
mod paste_id;
mod paste_data;

use paste_id::PasteID;
use paste_data::PasteData;
use std::io::{Error, Read};
use std::path::{Path, PathBuf};
use std::fs::{File, remove_file};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use rand::{Rng};
use rocket::response::{status, NamedFile, Redirect, Flash};
use rocket::request::{Form, FlashMessage};
use rocket::http::Status;
use rocket_contrib::Template;

static ERR_FILE_404: &'static str = "ERR_FILE_404";
static MSG_FILE_404: &'static str = "Could not find file";

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
               routes![get_static, index, upload, retrieve, remove])
        .launch()
}

fn remove_old_files(max_time_alive: Duration) -> std::io::Result<bool> {
    //TODO make this remove files according to preferred ttl
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
fn index(msg: Option<FlashMessage>) -> Template {
    let mut map: HashMap<&str, &str> = std::collections::HashMap::new();
    if let Some(msg_u) = msg {
        let code = msg_u.msg();
        if code == ERR_FILE_404 {
            map.insert("error", MSG_FILE_404);
        }
    }
    Template::render("index", &map)
}

#[derive(Clone)]
struct Paste {
    id: String,
    key: String,
    ttl: u64,
}

#[post("/", format="text/plain", data = "<paste>")]
fn upload(paste: PasteData) -> Result<Template, Redirect> {
    // TODO save all pastes somewhere with id and password and lifetime (use HashMap with own paste struct or db)
    let id = PasteID::new(24);
    let mut map = HashMap::new();
    match write_to_file(paste, &id) {
        Ok(res) => {
            let paste_id = format!("{}", id);
            let paste_key = generate_deletion_key();
            let new_paste = Paste { id: paste_id, key: paste_key, ttl: 60 * 60 * 24 * 7};
            // let new_paste_clone = new_paste.clone();
            map.insert("id", new_paste.id);
            map.insert("key", new_paste.key);
            map.insert("ttl", new_paste.ttl.to_string());
            map.insert("link", res.1.to_string());
            return Ok(Template::render("success", &map));
        },
        Err(res) => map.insert("error", res.to_string()),
    };
    Ok(Template::render("index", &map))
}

// #[put("/<id>", format="text/plain", data = "<paste>")]
// fn update(id: PasteID, paste: PasteData) -> std::io::Result<status::Custom<String>> {
//     write_to_file(paste, id)
// }

fn generate_deletion_key() -> String {
    let mut key = String::with_capacity(16);
    let mut rng = rand::thread_rng();
    for _ in 0..16 {
        key.push(paste_id::BASE62[rng.gen::<usize>() % 62] as char);
    }
    return key;
}

fn write_to_file(paste: PasteData, id: &PasteID) -> std::io::Result<status::Custom<String>> {
    let filename = format!("upload/{id}", id = id);
    let output = format!("{host}/{id}", host = "http://localhost:8000", id = id);

    paste.stream_to_file(Path::new(&filename))?;
    Ok(status::Custom(Status::Created, output))
}

#[get("/<id>", format="text/plain")]
fn retrieve(id: PasteID) -> Result<Template, Flash<Redirect>> {
    let filename = format!("upload/{id}", id = id);
    let mut data = String::new();
    if let Ok(mut f) = File::open(filename) {
        f.read_to_string(&mut data).expect("Unable to read string");
        let mut map = HashMap::new();
        map.insert("paste", data);
        return Ok(Template::render("paste", &map));
    };
    Err(Flash::error(Redirect::to("/"), ERR_FILE_404))
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
        // TODO change to generated paste key
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
