#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]
#![cfg_attr(test, plugin(stainless))]

extern crate rocket;
extern crate rocket_contrib;
extern crate rand;
#[macro_use]
extern crate lazy_static;
extern crate diesel;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate plib;

pub mod paste_id;
pub mod paste_data;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::remove_file;
use std::io::Error;

use plib::*;
use plib::models::Paste;

use diesel::prelude::*;
use diesel::pg::PgConnection;
use r2d2::{Pool, PooledConnection, GetTimeout};
use r2d2_diesel::ConnectionManager;

use rocket::request::{FromRequest, Outcome};
use rocket::Outcome::{Success, Failure};
use rocket::Request;
use rocket::http::Status;

fn main() {
    thread::spawn(|| {
        loop {
            let interval = 60;
            if let Err(err) = remove_old_files() {
                println!("Error: {}", err);
            }
            thread::sleep(Duration::from_secs(interval)) //TODO maybe make this better
        }
    });
    rocket::ignite()
        .catch(errors![routes::not_found, routes::too_large])
        .mount("/",
               routes![routes::get_static,
                       routes::index,
                       routes::upload,
                       routes::retrieve,
                       routes::remove])
        .launch()
}

lazy_static! {
    // TODO: are there race conditions? maybe cover with mutex
    pub static ref DB_POOL: Pool<ConnectionManager<PgConnection>> = create_db_pool();
}

pub struct DB(PooledConnection<ConnectionManager<PgConnection>>);

impl DB {
    pub fn conn(&self) -> &PgConnection {
        &*self.0
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for DB {
    type Error = GetTimeout;
    fn from_request(_: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        match DB_POOL.get() {
            Ok(conn) => Success(DB(conn)),
            Err(e) => Failure((Status::InternalServerError, e)),
        }
    }
}

fn remove_old_files() -> std::io::Result<usize> {
    use plib::schema::pastes::dsl::*;

    let mut count: usize = 0;

    if let Ok(pool_conn) = DB_POOL.get() {
        let ref conn = *pool_conn;
        let pastes_from_db = pastes.load::<Paste>(conn).expect("Error loading pastes");
        for paste in pastes_from_db {
            let file_string = "upload/".to_string() + &paste.get_id_cloned();
            let path = Path::new(&file_string);
            if !path.exists() {
                // file does not exist. Remove from db and then continue the loop
                println!("Removed zombie paste.");
                del_paste_from_db(paste.get_id_cloned());
                continue;
            }
            let metadata = try!(path.metadata());
            if let Ok(time) = metadata.modified() {
                let time_alive = time.elapsed().unwrap();
                if time_alive > Duration::from_secs(paste.get_ttl_u64()) {
                    if remove_file(path).is_ok() {
                        // also remove from db
                        count = count + del_paste_from_db(paste.get_id_cloned());
                        println!("Removed file of paste {}",
                                 path.file_name().unwrap().to_str().unwrap());
                    }
                }
            } else {
                return Err(Error::last_os_error());
            }
        }
    }
    Ok(count)
}

fn del_paste_from_db(p_id: String) -> usize {
    use plib::schema::pastes::dsl::*;

    if let Ok(pool_conn) = DB_POOL.get() {
        let ref conn = *pool_conn;
        diesel::delete(pastes.filter(id.like(p_id)))
            .execute(conn)
            .expect("Error deleting paste")
    } else {
        0
    }
}

mod routes {
    use std;
    use rocket;
    use diesel;
    use paste_id::{self, PasteID};
    use paste_data::PasteData;
    use plib::models::Paste;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::fs::{File, remove_file};
    use std::collections::HashMap;
    use rand::{self, Rng};
    use rocket::response::{status, NamedFile, Redirect, Flash};
    use rocket::request::{Form, FlashMessage};
    use rocket::http::Status;
    use rocket_contrib::Template;


    static ERR_FILE_404: &'static str = "ERR_FILE_404";
    static MSG_FILE_404: &'static str = "Could not find file";

    #[error(404)]
    pub fn not_found(req: &rocket::Request) -> Template {
        let mut map = HashMap::new();
        map.insert("path", req.uri().as_str());
        Template::render("404", &map)
    }

    #[error(413)]
    pub fn too_large() -> Template {
        let mut map = HashMap::new();
        map.insert("error", "Too large!");
        Template::render("index", &map)
    }

    #[get("/")]
    pub fn index(msg: Option<FlashMessage>) -> Template {
        let mut map: HashMap<&str, &str> = std::collections::HashMap::new();
        if let Some(msg_u) = msg {
            let code = msg_u.msg();
            if code == ERR_FILE_404 {
                map.insert("error", MSG_FILE_404);
            }
        }
        Template::render("index", &map)
    }

    #[post("/", format="text/plain", data = "<paste>")]
    pub fn upload(db: super::DB, paste: PasteData) -> Result<Template, Redirect> {
        // TODO add ttl option to Form and use it here
        use plib::schema::pastes;
        use diesel::LoadDsl;

        let p_id = PasteID::new(24);
        let mut map = HashMap::new();
        match write_to_file(paste, &p_id) {
            Ok(res) => {
                let paste_id = format!("{}", p_id);
                let paste_key = generate_deletion_key();
                let new_paste = Paste::new(paste_id, paste_key, 60 * 60 * 24 * 7); //TODO
                map.insert("id", new_paste.get_id_cloned());
                map.insert("key", new_paste.get_key_cloned());
                map.insert("ttl", new_paste.get_ttl_u64().to_string());
                map.insert("link", res.1.to_string());
                diesel::insert(&new_paste)
                    .into(pastes::table)
                    .get_result::<Paste>(db.conn())
                    .expect("Error saving new paste");
                return Ok(Template::render("success", &map));
            }
            Err(res) => map.insert("error", res.to_string()),
        };
        Ok(Template::render("index", &map))
    }

    // #[put("/<id>", format="text/plain", data = "<paste>")]
    // pub fn update(id: PasteID, paste: PasteData) -> std::io::Result<status::Custom<String>> {
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
        let output = format!("/{id}", id = id);

        paste.stream_to_file(Path::new(&filename))?;
        Ok(status::Custom(Status::Created, output))
    }

    #[get("/<id>", format="text/plain")]
    pub fn retrieve(id: PasteID) -> Result<Template, Flash<Redirect>> {
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
    pub struct PasteDel<'r> {
        paste_id: &'r str,
        paste_key: &'r str,
    }

    #[post("/remove", data = "<del_form>")]
    pub fn remove<'a>(del_form: Form<'a, PasteDel<'a>>) -> Template {
        let paste_del = del_form.get();
        let filename = format!("upload/{id}", id = paste_del.paste_id);
        let file = Path::new(&filename);
        let mut map = HashMap::new();
        if file.exists() {
            let key = paste_del.paste_key;
            if key == get_paste_key(paste_del.paste_id.into()) {
                if remove_file(file).is_ok() {
                    map.insert("success",
                               format!("Paste {id} removed", id = paste_del.paste_id));
                    super::del_paste_from_db(paste_del.paste_id.into());
                }
            } else {
                map.insert("error", "Invalid Paste ID or Key".into());
            }
        } else {
            map.insert("error", "Invalid Paste ID or Key".into());
        }
        Template::render("index", &map)
    }

    fn get_paste_key(paste_id: String) -> String {
        use plib::schema::pastes::dsl::*;
        use diesel::prelude::*;

        if let Ok(pool_conn) = super::DB_POOL.get() {
            let ref conn = *pool_conn;
            let k = pastes.find(paste_id)
                .first::<Paste>(conn)
                .expect("Error loading paste");
            k.get_key_cloned()
        } else {
            // db connection could not be established, return random key
            generate_deletion_key()
        }
    }

    #[get("/static/<file..>")]
    pub fn get_static(file: PathBuf) -> Option<NamedFile> {
        NamedFile::open(Path::new("static/").join(file)).ok()
    }
}

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use routes;
    use rocket;
    use rocket::testing::MockRequest;
    use rocket::http::{Status, Method, ContentType};

    describe! route_tests{
        before_each {
            let rocket = rocket::ignite()
                .catch(errors![routes::not_found, routes::too_large])
                .mount("/", routes![routes::get_static, routes::index, routes::upload, routes::retrieve, routes::remove]);
        }

        describe! status {
            before_each {
                let mut req = MockRequest::new(Method::Get, "/");
                let mut res = req.dispatch_with(&rocket);
                let body_str = res.body().and_then(|b| b.into_string()).expect("Result has no body!");
            }

            it "responds with status OK 200" {
                assert_eq!(res.status(), Status::Ok);
            }

            it "responds with no error" {
                assert!(!body_str.contains("Error"));
            }
        }

        describe! error404 {
            it "invalid url" {
                let mut req = MockRequest::new(Method::Get, "/invalid_url");
                let res = req.dispatch_with(&rocket);
                assert_eq!(res.status(), Status::NotFound);
            }
        }

        describe! post_paste {
            before_each {
                let mut base_req = MockRequest::new(Method::Post, "/");
            }

            it "basic paste" {
                let mut req = base_req.header(ContentType::Plain).body(&format!("paste={paste}", paste = "TODO"));
                let mut res = req.dispatch_with(&rocket);
                let body_str = res.body().and_then(|b| b.into_string()).expect("Result has no body!");

                assert!(body_str.contains("ID:"));
            }
        }
    }
}
