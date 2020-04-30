# juniper_aws_api_gateway_lambda

[![Build Status](https://travis-ci.org/selmeci/juniper_aws_api_gateway_lambda.svg?branch=master)](https://travis-ci.org/selmeci/juniper_aws_api_gateway_lambda)
[![Latest Version](https://img.shields.io/crates/v/juniper_aws_api_gateway_lambda.svg)](https://crates.io/crates/juniper_aws_api_gateway_lambda)
[![Docs](https://docs.rs/juniper_aws_api_gateway_lambda/badge.svg)](https://docs.rs/juniper_aws_api_gateway_lambda)

This repository contains the [AWS Lambda Runtime][AWS Lambda Runtime] on [AWS Api Gateway][AWS Api Gateway] integration for [Juniper][Juniper], a [GraphQL][GraphQL] implementation for Rust.

## Supported Versions

|    Version     |      Juniper      |
|:--------------:|:-----------------:|
|     0.1.X      |       0.14.2      |

## Documentation

For documentation, including guides and examples, check out [Juniper][Juniper].

A basic usage example can also be found in the [Api documentation][documentation].

## Examples

Check [examples/api_gateway][example] for example code of a working lambda with GraphQL handlers.

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
[example]: https://github.com/selmeci/juniper_aws_api_gateway_lambda/tree/master/examples/api_gateway