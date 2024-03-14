# `xray-lite-aws-sdk`

`xray-lite-aws-sdk` is an extension of [`xray-lite`](../) for [AWS SDK for Rust](https://aws.amazon.com/sdk-for-rust/).

## Installing `xray-lite-aws-sdk`

Add the following to your `Cargo.toml` file:

```toml
[dependencies]
xray-lite-aws-sdk = { git = "https://github.com/codemonger-io/xray-lite.git", tag = "aws-sdk-v0.0.1" }
```

## Usage

With this crate, you can easily add the X-Ray tracing capability to your AWS service requests through [AWS SDK for Rust](https://aws.amazon.com/sdk-for-rust/).
It utilizes the [interceptor](https://docs.rs/aws-smithy-runtime-api/latest/aws_smithy_runtime_api/client/interceptors/trait.Intercept.html) which can be attached to `CustomizableOperation` available via the `customize` method of any request builder; e.g., [`aws_sdk_s3::operation::get_object::builders::GetObjectFluentBuilder::customize`](https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/operation/get_object/builders/struct.GetObjectFluentBuilder.html#method.customize)

The following example shows how to report a subsegment for each attempt of the S3 GetObject operation:

```rust
use aws_config::BehaviorVersion;
use xray_lite::{Client, SubsegmentContext};
use xray_lite_aws_sdk::ContextExt as _;

async fn get_object_from_s3() {
    let xray_client = Client::from_lambda_env().unwrap();
    let xray_context = SubsegmentContext::from_lambda_env(xray_client).unwrap();

    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&config);
    s3_client
        .get_object()
        .bucket("the-bucket-name")
        .key("the-object-key")
        .customize()
        .interceptor(xray_context.intercept_operation("S3", "GetObject"))
        .send()
        .await
        .unwrap();
}
```

## API Documentation

<https://codemonger-io.github.io/xray-lite/api/xray_lite_aws_sdk/>