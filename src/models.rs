#[derive(Clone, Hash, Eq, PartialEq, Debug)] //Queryable
pub struct Paste {
    id: String,
    key: String,
    pub ttl: u32,
}

impl Paste {
    pub fn new(id: String, key: String, ttl: u32) -> Paste {
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
}
