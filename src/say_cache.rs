use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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

    pub fn to_vec(&self) -> Vec<UserId> {
        self.users.iter().map(|(x, _)| *x).collect()
    }
}

impl TypeMapKey for SayCache {
    type Value = SayCache;
}

impl From<(Vec<UserId>, &Path)> for SayCache {
    fn from((xs, path): (Vec<UserId>, &Path)) -> Self {
        let mut users = HashMap::new(TTLPolicy::new());

        for x in xs {
            users.insert(x, (), Duration::from_secs(3600 * 4));
        }

        Self {
            users,
            path: path.to_path_buf(),
        }
    }
}
