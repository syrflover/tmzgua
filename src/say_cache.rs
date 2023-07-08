use std::path::{Path, PathBuf};

use endorphin::{policy::TTLPolicy, HashMap};
use serenity::{model::id::UserId, prelude::TypeMapKey};

pub struct SayCache {
    pub users: HashMap<UserId, (), TTLPolicy>,
    pub path: PathBuf,
}

impl SayCache {
    pub fn new(cache_path: &Path) -> Self {
        Self {
            users: HashMap::new(TTLPolicy::new()),
            path: cache_path.to_path_buf(),
        }
    }
}

impl TypeMapKey for SayCache {
    type Value = SayCache;
}
