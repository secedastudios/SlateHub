use std::sync::LazyLock;
use surrealdb::{engine::remote::ws::Client, Surreal};

pub static DB: LazyLock<Surreal<Client>> = LazyLock::new(Surreal::init);
