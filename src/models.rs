use super::schema::pastes;
#[derive(Clone, Hash, Eq, PartialEq, Debug, Queryable, Insertable)]
#[table_name="pastes"]
pub struct Paste {
    id: String,
    key: String,
    ttl: i32,
}

impl Paste {
    pub fn new(id: String, key: String, ttl: i32) -> Paste {
        Paste {
            id: id,
            key: key,
            ttl: ttl,
        }
    }

    pub fn get_id_cloned(&self) -> String {
        self.id.clone()
    }

    pub fn get_key_cloned(&self) -> String {
        self.key.clone()
    }

    pub fn get_ttl(&self) -> u64 {
        // TODO
        // 60 * 60 * 24 * 7
        30
    }
}
