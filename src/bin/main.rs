#![feature(plugin, custom_derive)]
#![plugin(rocket_codegen)]
#![cfg_attr(feature = "dev", allow(unstable_features))]
#![cfg_attr(feature = "dev", plugin(clippy))]

extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
extern crate rand;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
extern crate diesel;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate plib;
extern crate time;

pub mod paste_id;
pub mod paste_data;

use diesel::pg::PgConnection;
use diesel::prelude::*;
use plib::*;
use plib::models::Paste;
use r2d2::{Pool, PooledConnection, GetTimeout};
use r2d2_diesel::ConnectionManager;
use rocket::Outcome::{Success, Failure};
use rocket::Request;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use std::io::*;
use std::io::Error;
use std::thread;
use std::time::Duration;

fn main() {
    thread::spawn(|| {
        loop {
            let interval = 420; //TODO time interval?
            if let Err(err) = remove_old_pastes() {
                println!("Error: {}", err);
            }
            thread::sleep(Duration::from_secs(interval))
        }
    });
    rocket::ignite()
        .catch(errors![routes::not_found, routes::too_large])
        .mount("/",
               routes![routes::get_static,
                       routes::index,
                       routes::upload,
                       routes::upload_json,
                       routes::retrieve,
                       routes::retrieve_json,
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

fn remove_old_pastes() -> Result<usize> {
    use plib::schema::pastes::dsl::*;

    let mut count: usize = 0;

    let pool_conn = DB_POOL.get();
    if let Err(err) = pool_conn {
        return Err(Error::new(ErrorKind::Other, err.to_string()));
    }

    let curr_time = time::get_time().sec;
    let conn = &(*pool_conn.expect("Could not unwrap pooled connection!"));
    for p in pastes.load::<Paste>(conn).expect("Error loading pastes") {
        let creation_time = p.get_created();
        let max_time = creation_time + p.get_ttl_i64();
        if curr_time > max_time {
            //remove from db
            count += del_paste_from_db(p.get_id_cloned());
            println!("Paste with id {} expired. Removed it.", p.get_id_cloned());
        }      
    }
    Ok(count)
}

fn del_paste_from_db(p_id: String) -> usize {
    use plib::schema::pastes::dsl::*;

    if let Ok(pool_conn) = DB_POOL.get() {
        let conn = &(*pool_conn);
        diesel::delete(pastes.filter(id.like(p_id))).execute(conn).expect("Error deleting paste")
    } else {
        0
    }
}

mod routes {
    use DB;
    use diesel;
    use paste_data::PasteData;
    use paste_id::{self, PasteID};
    use plib::models::Paste;
    use rand::{self, Rng};
    use rocket;
    use rocket::request::{Form, FlashMessage};
    use rocket::response::{NamedFile, Redirect, Flash};
    use rocket_contrib::{JSON, Value, Template};
    use std;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use time;

    static ERR_FILE_404: &'static str = "ERR_FILE_404";
    static MSG_FILE_404: &'static str = "Could not find file";
    static MSG_DB_ERR: &'static str = "Cannot access database";

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

    fn save_paste<'a>(db: &super::DB, paste: PasteData) -> HashMap<&'a str, String> {
        // TODO add ttl option to Form and use it here
        use plib::schema::pastes;
        use diesel::LoadDsl;

        let p_id = PasteID::new();
        let mut map = HashMap::new();
        let content = paste.get_content_cloned();
        let paste_id = format!("{}", p_id);
        let paste_key = generate_deletion_key();
        let current_time = time::get_time();
        let paste_creation = current_time.sec;
        let paste_ttl = 60 * 60 * 24 * 7;
        let new_paste = Paste::new(paste_id,
                                   paste_key,
                                   paste_ttl,
                                   paste_creation,
                                   content);
        let save_result =
            diesel::insert(&new_paste).into(pastes::table).get_result::<Paste>(db.conn());
        match save_result {
            Ok(res) => {
                map.insert("id", new_paste.get_id_cloned());
                map.insert("key", new_paste.get_key_cloned());
                map.insert("ttl", new_paste.get_ttl_i64().to_string());
                map.insert("link", res.get_id_cloned().to_string());
            }
            Err(res) => {
                map.insert("error", res.to_string());
            }
        };
        map
    }

    #[post("/", format="text/plain", data = "<paste>")]
    pub fn upload(db: super::DB, paste: PasteData) -> Result<Template, Redirect> {
        let map = save_paste(&db, paste);
        if map.contains_key("error") {
            return Ok(Template::render("index", &map));
        }
        Ok(Template::render("success", &map))
    }

    #[post("/", format="application/json", data = "<paste>")]
    pub fn upload_json(db: super::DB, paste: JSON<PasteData>) -> JSON<Value> {
        JSON(json!(save_paste(&db, paste.0)))
    }

    fn generate_deletion_key() -> String {
        let mut key = String::with_capacity(16);
        let mut rng = rand::thread_rng();
        for _ in 0..16 {
            key.push(paste_id::BASE62[rng.gen::<usize>() % 62] as char);
        }
        key
    }

    #[get("/<id>", rank=3)]
    pub fn retrieve(id: PasteID) -> Result<Template, Flash<Redirect>> {
        let db = match super::DB_POOL.get() {
            Ok(conn) => DB(conn),
            Err(_) => return Err(Flash::error(Redirect::to("/"), MSG_DB_ERR)),
        };
        if let Ok(data) = get_data(&db, id.id()) {
            let mut map = HashMap::new();
            map.insert("paste", data);
            return Ok(Template::render("paste", &map));
        }
        Err(Flash::error(Redirect::to("/"), ERR_FILE_404))
    }

    #[get("/<id>", format="application/json", rank=2)]
    pub fn retrieve_json(id: PasteID) -> JSON<Value> {
        let db = match super::DB_POOL.get() {
            Ok(conn) => DB(conn),
            Err(_) => {
                return JSON(json!({
                                      "error": MSG_DB_ERR
                                  }));
            }
        };
        if let Ok(data) = get_data(&db, id.id()) {
            JSON(json!({
                           "paste": data
                       }))
        } else {
            JSON(json!({
                           "error": MSG_FILE_404
                       }))
        }
    }

    fn get_data(db: &super::DB, paste_id: String) -> Result<String, String> {
        use plib::schema::pastes::dsl::*;
        use diesel::*;

        let db_res = pastes.filter(id.like(paste_id)).limit(1).load::<Paste>(db.conn());

        match db_res {
            Ok(res) => {
                if res.len() < 1 {
                    Err(ERR_FILE_404.into())
                } else {
                    Ok(res[0].get_paste_cloned())
                }},
            Err(_) => Err(ERR_FILE_404.into()),
        }
    }

    #[derive(FromForm)]
    pub struct PasteDel<'r> {
        paste_id: &'r str,
        paste_key: &'r str,
    }

    #[post("/remove", data = "<del_form>")]
    pub fn remove<'a>(del_form: Form<'a, PasteDel<'a>>) -> Template {
        let paste_del = del_form.get();
        let mut map = HashMap::new();
        if paste_del.paste_key == get_paste_key(paste_del.paste_id.into()) {
            if super::del_paste_from_db(paste_del.paste_id.into()) > 0 {  
                map.insert("success", format!("Paste {id} removed", id = paste_del.paste_id));
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
            let conn = &(*pool_conn);
            let k = pastes.find(paste_id).first::<Paste>(conn).expect("Error loading paste");
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

//TODO: more tests
#[cfg(test)]
#[allow(unused_variables, unused_mut, dead_code)]
mod tests {
    use rocket;
    use rocket::http::{Status, Method, ContentType};
    use rocket::testing::MockRequest;
    use routes;

    fn post_data_req(size: usize, rocket: &rocket::Rocket) -> (Status, String) {
        let mut paste_content = String::new();
        for _ in 0..(size / 2) {
            paste_content += "XX";
        }
        let mut req = MockRequest::new(Method::Post, "/")
            .header(ContentType::new("text", "plain"))
            .body(&format!("paste={paste}", paste = paste_content));
        let mut res = req.dispatch_with(rocket);
        let body_str = res.body().and_then(|b| b.into_string()).expect("Result has no body!");
        (res.status(), body_str)
    }

    fn mount_rocket() -> rocket::Rocket {
        rocket::ignite()
            .catch(errors![routes::not_found, routes::too_large])
            .mount("/",
                   routes![routes::get_static,
                           routes::index,
                           routes::upload,
                           routes::upload_json,
                           routes::retrieve,
                           routes::retrieve_json,
                           routes::remove])
    }

    #[test]
    fn test_index() {
        let rocket = mount_rocket();
        let mut req = MockRequest::new(Method::Get, "/");
        let mut res = req.dispatch_with(&rocket);
        let body_str = res.body().and_then(|b| b.into_string()).expect("Result has no body!");

        assert_eq!(res.status(), Status::Ok);
        assert!(!body_str.contains("Error"));
    }

    #[test]
    fn test_get_invalid() {
        let rocket = mount_rocket();
        let mut req = MockRequest::new(Method::Get, "/pSz062HQr66TgmluU2hXK7He"); //"random" pasteid
        let mut res = req.dispatch_with(&rocket);
        assert_eq!(res.status(), Status::SeeOther);
    }

    #[test]
    fn test_404() {
        let rocket = mount_rocket();
        let mut req = MockRequest::new(Method::Get, "/invalid_url");
        let res = req.dispatch_with(&rocket);
        assert_eq!(res.status(), Status::NotFound);
    }

    #[test]
    fn test_post() {
        let rocket = mount_rocket();
        let (status, body_str) = post_data_req(42, &rocket);
        assert_eq!(status, Status::Ok);
        assert!(body_str.contains("ID:"));
    }

}
