use crate::types::element::Element;
use crate::types::User;
use std::collections::HashMap;
use std::sync::Mutex;

///
/// Context for Juniper
///
pub struct Context {
    // Use your real database pool here.
    pub pool: Mutex<HashMap<String, Element>>,
    // author of request
    pub author: User,
}

impl juniper::Context for Context {}

impl Context {
    pub fn new<S: Into<String>>(id: S) -> Self {
        let mut pool = HashMap::new();
        // insert first element into DB
        pool.insert(
            "1".into(),
            Element {
                hash: "1".into(),
                author: User::new("demo1"),
            },
        );
        Self {
            pool: Mutex::new(pool),
            author: User::new(id),
        }
    }
}
