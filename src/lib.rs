/*!

# juniper_aws_api_gateway_lambda

[![Build Status](https://travis-ci.org/selmeci/juniper_aws_api_gateway_lambda.svg?branch=master)](https://travis-ci.org/selmeci/juniper_aws_api_gateway_lambda)
[![Latest Version](https://img.shields.io/crates/v/juniper_aws_api_gateway_lambda.svg)](https://crates.io/crates/juniper_aws_api_gateway_lambda)
[![Docs](https://docs.rs/juniper_aws_api_gateway_lambda/badge.svg)](https://docs.rs/juniper_aws_api_gateway_lambda)

This repository contains the [AWS Lambda Runtime][AWS Lambda Runtime] on [AWS Api Gateway][AWS Api Gateway] integration for [Juniper][Juniper], a [GraphQL][GraphQL] implementation for Rust.

## Documentation

For documentation, including guides and examples, check out [Juniper][Juniper].

A basic usage example can also be found in the [Api documentation][documentation].

## Examples

Check [examples/api_gateway.rs][example] for example code of a working lambda with GraphQL handlers.

## Links

* [Juniper][Juniper]
* [Api Reference][documentation]
* [AWS Lambda Runtime][AWS Lambda Runtime]

## License

This project is under the MIT license.

[AWS Api Gateway]: https://aws.amazon.com/api-gateway/
[AWS Lambda Runtime]: https://github.com/awslabs/aws-lambda-rust-runtime
[Juniper]: https://github.com/graphql-rust/juniper
[GraphQL]: http://graphql.org
[documentation]: https://docs.rs/juniper_aws_api_gateway_lambda
[example]: https://github.com/graphql-rust/juniper/blob/master/juniper_rocket/examples/rocket_server.rs

*/

extern crate serde_derive;

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use aws_lambda_events::event::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use failure::Fail;
use http::{header, method::Method, status::StatusCode};
use juniper::{
    http as juniper_http, DefaultScalarValue, FieldError, GraphQLType, InputValue, RootNode,
    ScalarRefValue, ScalarValue,
};
use juniper_http::GraphQLRequest as GqlR;
use lambda::Handler;
use maplit::hashmap;
use percent_encoding::percent_decode_str;
use serde::Deserialize;

#[derive(Debug, Fail, Eq, PartialEq)]
pub enum Error {
    #[fail(display = "UnknownMethod")]
    UnknownMethod(String),
    #[fail(display = "InvalidMethod")]
    InvalidMethod(Method),
    #[fail(display = "Missing query argument")]
    MissingQuery,
    #[fail(display = "Missing post body")]
    MissingPostBody,
    #[fail(display = "Invalid body")]
    InvalidBody,
    #[fail(display = "Prohibit extra field")]
    ProhibitExtraField(String),
    #[fail(display = "Query parameter must not occur more than once")]
    MultipleQueryParameter,
    #[fail(display = "Operation name parameter must not occur more than once")]
    MultipleOperationNameParameter,
    #[fail(display = "Variables parameter must not occur more than once")]
    MultipleVariablesParameter,
    #[fail(display = "Invalid variables parameter")]
    InvalidVariablesParameter,
    #[fail(display = "Invalid query parameter encoding")]
    InvalidQueryParameterEncoding,
}

#[derive(Debug, serde_derive::Deserialize, PartialEq)]
#[serde(untagged)]
#[serde(bound = "InputValue<S>: Deserialize<'de>")]
enum GraphQLBatchRequest<S = DefaultScalarValue>
where
    S: ScalarValue,
{
    Single(juniper_http::GraphQLRequest<S>),
    Batch(Vec<juniper_http::GraphQLRequest<S>>),
}

impl<S> GraphQLBatchRequest<S>
where
    S: ScalarValue,
    for<'b> &'b S: ScalarRefValue<'b>,
{
    pub fn execute<'a, CtxT, QueryT, MutationT>(
        &'a self,
        root_node: &'a RootNode<QueryT, MutationT, S>,
        context: &CtxT,
    ) -> GraphQLBatchResponse<'a, S>
    where
        QueryT: GraphQLType<S, Context = CtxT>,
        MutationT: GraphQLType<S, Context = CtxT>,
    {
        match self {
            &GraphQLBatchRequest::Single(ref request) => {
                GraphQLBatchResponse::Single(request.execute(root_node, context))
            }
            &GraphQLBatchRequest::Batch(ref requests) => GraphQLBatchResponse::Batch(
                requests
                    .iter()
                    .map(|request| request.execute(root_node, context))
                    .collect(),
            ),
        }
    }

    pub fn operation_names(&self) -> Vec<Option<&str>> {
        match self {
            GraphQLBatchRequest::Single(req) => vec![req.operation_name()],
            GraphQLBatchRequest::Batch(reqs) => {
                reqs.iter().map(|req| req.operation_name()).collect()
            }
        }
    }
}

#[derive(serde_derive::Serialize)]
#[serde(untagged)]
enum GraphQLBatchResponse<'a, S = DefaultScalarValue>
where
    S: ScalarValue,
{
    Single(juniper_http::GraphQLResponse<'a, S>),
    Batch(Vec<juniper_http::GraphQLResponse<'a, S>>),
}

impl<'a, S> GraphQLBatchResponse<'a, S>
where
    S: ScalarValue,
{
    fn is_ok(&self) -> bool {
        match self {
            &GraphQLBatchResponse::Single(ref response) => response.is_ok(),
            &GraphQLBatchResponse::Batch(ref responses) => responses
                .iter()
                .fold(true, |ok, response| ok && response.is_ok()),
        }
    }
}

fn method(req: &ApiGatewayProxyRequest) -> Result<Method, Error> {
    let raw_method = req.http_method.to_owned().unwrap_or_default();
    match Method::try_from(raw_method.as_str()) {
        Ok(method) => Ok(method),
        Err(_err) => Err(Error::UnknownMethod(raw_method)),
    }
}

fn response(
    status_code: StatusCode,
    content_type: String,
    body: String,
) -> ApiGatewayProxyResponse {
    ApiGatewayProxyResponse {
        status_code: status_code.as_u16() as i64,
        multi_value_headers: HashMap::with_capacity(0),
        headers: hashmap! {header::CONTENT_TYPE.to_string() => content_type},
        is_base64_encoded: Some(false),
        body: Some(body),
    }
}

fn html(body: String) -> ApiGatewayProxyResponse {
    response(StatusCode::OK, "text/html".into(), body)
}

fn json(status_code: StatusCode, body: String) -> ApiGatewayProxyResponse {
    response(status_code, "application/json".into(), body)
}

fn url_decode(input: &str) -> Result<String, Error> {
    match percent_decode_str(input).decode_utf8() {
        Ok(q) => Ok(q.to_string()),
        Err(_) => Err(Error::InvalidQueryParameterEncoding),
    }
}

#[serde(deny_unknown_fields)]
#[derive(Deserialize, Clone, PartialEq, Debug)]
struct GetGraphQLRequest {
    query: String,
    operation_name: Option<String>,
    variables: Option<String>,
}

impl<S> TryFrom<GetGraphQLRequest> for GqlR<S>
where
    S: ScalarValue,
{
    type Error = Error;

    fn try_from(get_req: GetGraphQLRequest) -> Result<Self, Self::Error> {
        let GetGraphQLRequest {
            query,
            operation_name,
            variables,
        } = get_req;
        let variables = match variables {
            Some(variables) => match serde_json::from_str(&variables) {
                Ok(variables) => Some(variables),
                Err(_) => return Err(Error::InvalidVariablesParameter),
            },
            None => None,
        };
        Ok(Self::new(query, operation_name, variables))
    }
}

/// Simple wrapper around an incoming GraphQL request
///
/// See the `http` module for more information. This type can be constructed
/// automatically from both GET and POST routes by implementing the `FromForm`
/// and `FromData` traits.
#[derive(Debug, PartialEq)]
pub struct GraphQLRequest<S = DefaultScalarValue>(GraphQLBatchRequest<S>)
where
    S: ScalarValue;

impl<S> GraphQLRequest<S>
where
    S: ScalarValue,
{
    fn from_get(req: &ApiGatewayProxyRequest) -> Result<Self, Error> {
        let mut query: Option<String> = None;
        let mut operation_name: Option<String> = None;
        let mut variables: Option<String> = None;
        let query_string_parameters = &req.multi_value_query_string_parameters;
        for (key, value) in query_string_parameters {
            match key.as_str() {
                "query" => {
                    if value.is_empty() {
                        return Err(Error::MissingQuery);
                    } else if value.len() > 1 {
                        return Err(Error::MultipleQueryParameter);
                    } else {
                        query.replace(url_decode(&value[0])?);
                    }
                }
                "operation_name" => {
                    if value.len() > 1 {
                        return Err(Error::MultipleOperationNameParameter);
                    } else {
                        operation_name.replace(url_decode(&value[0])?);
                    }
                }
                "variables" => {
                    if value.len() > 1 {
                        return Err(Error::MultipleVariablesParameter);
                    } else {
                        variables.replace(url_decode(&value[0])?);
                    }
                }
                _ => return Err(Error::ProhibitExtraField(key.to_owned())),
            }
        }
        if query.is_none() {
            return Err(Error::MissingQuery);
        };
        let req = GetGraphQLRequest {
            variables,
            operation_name,
            query: query.unwrap(),
        };
        Ok(Self(GraphQLBatchRequest::Single(req.try_into()?)))
    }

    fn from_post(req: &ApiGatewayProxyRequest) -> Result<Self, Error> {
        if let Some(body) = &req.body {
            match serde_json::from_str::<GetGraphQLRequest>(body) {
                Ok(req) => Ok(Self(GraphQLBatchRequest::Single(req.try_into()?))),
                Err(_) => match serde_json::from_str::<Vec<GetGraphQLRequest>>(body) {
                    Ok(requests) => {
                        let maybe_requests: Vec<Result<GqlR<S>, Error>> =
                            requests.into_iter().map(|req| req.try_into()).collect();
                        let mut requests = Vec::with_capacity(maybe_requests.len());
                        for maybe_request in maybe_requests {
                            match maybe_request {
                                Ok(request) => requests.push(request),
                                Err(err) => return Err(err),
                            }
                        }
                        Ok(Self(GraphQLBatchRequest::Batch(requests)))
                    }
                    Err(_) => Err(Error::InvalidBody),
                },
            }
        } else {
            Err(Error::MissingPostBody)
        }
    }
}

impl<S> GraphQLRequest<S>
where
    S: ScalarValue,
    for<'b> &'b S: ScalarRefValue<'b>,
{
    /// Execute an incoming GraphQL query
    pub fn execute<CtxT, QueryT, MutationT>(
        &self,
        root_node: &RootNode<QueryT, MutationT, S>,
        context: &CtxT,
    ) -> ApiGatewayProxyResponse
    where
        QueryT: GraphQLType<S, Context = CtxT>,
        MutationT: GraphQLType<S, Context = CtxT>,
    {
        let response = self.0.execute(root_node, context);
        let status_code = if response.is_ok() {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        };
        let body = serde_json::to_string(&response).unwrap();

        json(status_code, body)
    }

    /// Returns the operation names associated with this request.
    ///
    /// For batch requests there will be multiple names.
    pub fn operation_names(&self) -> Vec<Option<&str>> {
        self.0.operation_names()
    }
}

/// Constructs an error response outside of the normal execution flow
pub fn error(error: FieldError) -> ApiGatewayProxyResponse {
    let response = juniper_http::GraphQLResponse::error(error);
    let body = serde_json::to_string(&response).unwrap();
    json(StatusCode::BAD_REQUEST, body)
}

/// Constructs a custom response outside of the normal execution flow
///
/// This is intended for highly customized integrations and should only
/// be used as a last resort. For normal juniper use, use the response
/// from GraphQLRequest::execute_sync(..).
pub fn custom(status_code: StatusCode, response: serde_json::Value) -> ApiGatewayProxyResponse {
    let body = serde_json::to_string(&response).unwrap();
    json(status_code, body)
}

impl<S> TryFrom<ApiGatewayProxyRequest> for GraphQLRequest<S>
where
    S: ScalarValue + Send + Sync,
{
    type Error = Error;

    fn try_from(req: ApiGatewayProxyRequest) -> Result<Self, Self::Error> {
        match method(&req)? {
            Method::GET => Ok(Self::from_get(&req)?),
            Method::POST => Ok(Self::from_post(&req)?),
            raw_method => Err(Error::InvalidMethod(raw_method)),
        }
    }
}

/// Generate an HTML page containing GraphiQL
pub fn graphiql_source(graphql_endpoint_url: &str) -> ApiGatewayProxyResponse {
    html(juniper::http::graphiql::graphiql_source(
        graphql_endpoint_url,
    ))
}

/// Generate an HTML page containing GraphQL Playground
pub fn playground_source(graphql_endpoint_url: &str) -> ApiGatewayProxyResponse {
    html(juniper::http::playground::playground_source(
        graphql_endpoint_url,
    ))
}

/// Aws Api Gateway GraphQL Handler for GET and POST requests
pub struct GraphQLHandler<CtxT, QueryT, MutationT, S>
where
    S: ScalarValue + Send + Sync + 'static,
    for<'b> &'b S: ScalarRefValue<'b>,
    CtxT: Send + Sync + 'static,
    QueryT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    MutationT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync,
{
    root_node: Arc<RootNode<'static, QueryT, MutationT, S>>,
    context: Arc<CtxT>,
}

impl<CtxT, QueryT, MutationT, S> GraphQLHandler<CtxT, QueryT, MutationT, S>
where
    S: ScalarValue + Send + Sync + 'static,
    for<'b> &'b S: ScalarRefValue<'b>,
    CtxT: Send + Sync + 'static,
    QueryT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    MutationT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync,
{
    pub fn new(root_node: RootNode<'static, QueryT, MutationT, S>, context: CtxT) -> Self {
        Self {
            root_node: Arc::new(root_node),
            context: Arc::new(context),
        }
    }
}

impl<CtxT, QueryT, MutationT, S> Handler<ApiGatewayProxyRequest, ApiGatewayProxyResponse>
    for GraphQLHandler<CtxT, QueryT, MutationT, S>
where
    S: ScalarValue + Send + Sync + 'static,
    for<'b> &'b S: ScalarRefValue<'b>,
    CtxT: Send + Sync + 'static,
    QueryT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    MutationT: GraphQLType<S, Context = CtxT> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync,
{
    type Error = Error;
    type Fut = Pin<Box<dyn Future<Output = Result<ApiGatewayProxyResponse, Self::Error>> + Send>>;

    fn call(&mut self, req: ApiGatewayProxyRequest) -> Self::Fut {
        let root_node = Arc::clone(&self.root_node);
        let context = Arc::clone(&self.context);
        Box::pin(async move {
            let gql_req = GraphQLRequest::<S>::try_from(req)?;
            let gql_res = gql_req.execute(&root_node, &context);
            Ok(gql_res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_request() -> ApiGatewayProxyRequest {
        let identity = json!({});
        let context = json!({
            "identity": identity,
            "operation_name": "test",
        });
        let value = json!({
            "requestContext": context,
            "isBase64Encoded": false
        });
        let req: ApiGatewayProxyRequest =
            serde_json::from_value(value).expect("ApiGatewayProxyRequest");
        req
    }

    fn check_error(req: ApiGatewayProxyRequest, error: Error) {
        let result: Result<GraphQLRequest, Error> = GraphQLRequest::try_from(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), error);
    }

    #[test]
    fn empty_get() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        check_error(req, Error::MissingQuery);
    }

    #[test]
    fn empty_post() {
        let mut req = empty_request();
        req.http_method = Some("POST".into());
        req.body = Some("".into());
        check_error(req, Error::InvalidBody);
    }

    #[test]
    fn no_query() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "operation_name".into() => vec!["foo".into()],
            "variables".into() => vec!["{}".into()],
        };
        check_error(req, Error::MissingQuery);
    }

    #[test]
    fn prohibited_extra_field() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["".into()],
            "operation_name".into() => vec!["foo".into()],
            "variables".into() => vec!["{}".into()],
            "extra".into() => vec!["field".into()],
        };
        check_error(req, Error::ProhibitExtraField(String::from("extra")));
    }

    #[test]
    fn duplicate_query() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["".into(), "".into()],
            "operation_name".into() => vec!["foo".into()],
            "variables".into() => vec!["{}".into()],
        };
        check_error(req, Error::MultipleQueryParameter);
    }

    #[test]
    fn duplicate_operation_name() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["".into()],
            "operation_name".into() => vec!["foo".into(), "".into()],
            "variables".into() => vec!["{}".into()],
        };
        check_error(req, Error::MultipleOperationNameParameter);
    }

    #[test]
    fn duplicate_variables() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["".into()],
            "operation_name".into() => vec!["foo".into()],
            "variables".into() => vec!["{}".into(), "{}".into()],
        };
        check_error(req, Error::MultipleVariablesParameter);
    }

    #[test]
    fn variables_invalid_json() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["".into()],
            "operation_name".into() => vec!["foo".into()],
            "variables".into() => vec!["{a: 1,}".into()],
        };
        check_error(req, Error::InvalidVariablesParameter);
    }

    #[test]
    fn variables_valid_json() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["test".into()],
            "variables".into() => vec![r#"{"foo":"bar"}"#.into()],
        };
        let result = GraphQLRequest::try_from(req);
        assert!(result.is_ok());
        let variables = serde_json::from_str::<InputValue>(r#"{"foo":"bar"}"#).unwrap();
        let expected = GraphQLRequest(GraphQLBatchRequest::Single(
            juniper_http::GraphQLRequest::new("test".into(), None, Some(variables)),
        ));
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn variables_encoded_json() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["test".into()],
            "variables".into() => vec![r#"{"foo": "x%20y%26%3F+z"}"#.into()],
        };
        let result = GraphQLRequest::try_from(req);
        assert!(result.is_ok());
        let variables = ::serde_json::from_str::<InputValue>(r#"{"foo":"x y&?+z"}"#).unwrap();
        let expected = GraphQLRequest(GraphQLBatchRequest::Single(
            juniper_http::GraphQLRequest::new("test".into(), None, Some(variables)),
        ));
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn url_decode() {
        let mut req = empty_request();
        req.http_method = Some("GET".into());
        req.multi_value_query_string_parameters = hashmap! {
            "query".into() => vec!["%25foo%20bar+baz%26%3F".into()],
            "operation_name".into() => vec!["test".into()],
        };
        let result = GraphQLRequest::<DefaultScalarValue>::try_from(req);
        assert!(result.is_ok());
        let expected = GraphQLRequest(GraphQLBatchRequest::Single(
            juniper_http::GraphQLRequest::new("%foo bar+baz&?".into(), Some("test".into()), None),
        ));
        assert_eq!(result.unwrap(), expected);
    }
}
