///
/// GraphQL type for a user
///
#[derive(juniper::GraphQLObject, Clone, Default, Debug)]
pub struct User {
    /// Common name of user
    cn: String,
    /// Surname if user
    sn: String,
    /// Email
    email: String,
    /// unique identification of user
    uid: String,
}

impl User {
    pub fn new<S: Into<String>>(uid: S) -> Self {
        Self {
            uid: uid.into(),
            ..Default::default()
        }
    }
}
