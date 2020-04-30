extern crate juniper;

use crate::context::Context;
use crate::types::element::schema;
use juniper_aws_api_gateway_lambda::GraphQLHandler;

mod context;
mod types;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let context = Context::new("demo_user");
    let root_node = schema();
    let handler = GraphQLHandler::new(root_node, context);
    lambda::run(handler).await?;
    Ok(())
}
