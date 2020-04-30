use crate::context::Context;
use crate::types::User;
use juniper::FieldResult;

///
/// GraphQL type for element
///
#[derive(juniper::GraphQLObject, Clone, Debug)]
pub struct Element {
    /// unique identification of element in eDocu
    pub hash: String,
    /// author of element
    pub author: User,
}

///
/// GraphQL type for a new element
///
#[derive(juniper::GraphQLInputObject)]
struct NewElement {
    /// unique identification of element in eDocu
    hash: String,
}

pub struct Query;

#[juniper::object(Context = Context)]
impl Query {
    fn api_version() -> String {
        String::from("1.0")
    }

    ///
    /// Get element by hash from DB
    ///
    fn element(context: &Context, hash: String) -> FieldResult<Option<Element>> {
        if let Some(element) = context.pool.lock().unwrap().get(&hash) {
            Ok(Some(element.to_owned()))
        } else {
            Ok(None)
        }
    }

    ///
    /// Get all element from DB
    ///
    fn elements(context: &Context) -> FieldResult<Vec<Element>> {
        let db = context.pool.lock().unwrap();
        let mut result = Vec::new();
        for element in db.values() {
            result.push(element.to_owned())
        }
        Ok(result)
    }
}

pub struct Mutation;

#[juniper::object(Context = Context)]
impl Mutation {
    ///
    /// Create new element in DB
    ///
    fn create_element(context: &Context, new_element: NewElement) -> FieldResult<Element> {
        let mut db = context.pool.lock().unwrap();
        let element = Element {
            author: context.author.to_owned(),
            hash: new_element.hash,
        };
        db.insert(element.hash.to_owned(), element.to_owned());
        Ok(element)
    }
}

type Schema = juniper::RootNode<'static, Query, Mutation>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation)
}
