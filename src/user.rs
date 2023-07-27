use std::fmt::Display;

use petname::Petnames;
use rand::{rngs::StdRng, SeedableRng};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

static USER_HANDLE_GEN: OnceCell<Mutex<UserHandleGenerator>> = OnceCell::const_new();

pub struct UserHandleGenerator {
    gen: Petnames<'static>,
    rng: StdRng,
}

impl UserHandleGenerator {
    pub fn new() -> Self {
        Self {
            gen: Petnames::medium(),
            rng: StdRng::from_entropy(),
        }
    }

    pub fn _from_seed(seed: u64) -> Self {
        Self {
            gen: Petnames::medium(),
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn gen(&mut self) -> UserHandle {
        let handle = self.gen.generate(&mut self.rng, 3, "-");
        UserHandle(handle)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserHandle(String);

impl AsRef<str> for UserHandle {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl UserHandle {
    pub async fn new() -> Self {
        USER_HANDLE_GEN
            .get_or_init(|| async { Mutex::new(UserHandleGenerator::new()) })
            .await
            .lock()
            .await
            .gen()
    }
    pub fn _from_gen(generator: &mut UserHandleGenerator) -> Self {
        generator.gen()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    id: Uuid,
    handle: UserHandle,
}

impl User {
    pub async fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            handle: UserHandle::new().await,
        }
    }

    pub const fn id(&self) -> &Uuid {
        &self.id
    }

    pub const fn handle(&self) -> &UserHandle {
        &self.handle
    }
}

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.handle.as_ref())
    }
}
