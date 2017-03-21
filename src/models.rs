use super::schema::pastes;
#[derive(Debug, Queryable, Insertable)]
#[table_name="pastes"]
pub struct Paste {
    id: String,
    key: String,
    ttl: i32,
    created: i64,
    paste: String,
}

impl Paste {
    pub fn new(id: String, key: String, ttl: i32, created: i64, paste: String) -> Paste {
        Paste {
            id: id,
            key: key,
            ttl: ttl,
            created: created,
            paste: paste,
        }
    }

    pub fn get_id_cloned(&self) -> String {
        self.id.clone()
    }

    pub fn get_key_cloned(&self) -> String {
        self.key.clone()
    }

    pub fn get_ttl_u64(&self) -> u64 {
        self.ttl as u64
    }

    pub fn get_created(&self) -> i64 {
        self.created
    }

    pub fn get_paste_cloned(&self) -> String {
        self.paste.clone()
    }
}
