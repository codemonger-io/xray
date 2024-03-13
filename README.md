# xray-lite

> [AWS X-Ray](https://aws.amazon.com/xray/) daemon client for Rust applications on [AWS Lambda](https://aws.amazon.com/lambda/)

## Installing

Add the following to your `Cargo.toml` file:

```toml
[dependencies]
xray-lite = { git = "https://github.com/codemonger-io/xray-lite.git", tag = "v0.0.6" }
```

## Usage

### Subsegment of AWS service operation

Here is an example to record a subsegment of an AWS service operation within a Lambda function invocation instrumented with AWS X-Ray:

```rust
use xray_lite::{AwsNamespace, Client, Context, SubsegmentContext};

fn main() {
   // reads AWS_XRAY_DAEMON_ADDRESS
   let client = Client::from_lambda_env().unwrap();
   // reads _X_AMZN_TRACE_ID
   let context = SubsegmentContext::from_lambda_env(client).unwrap();

   do_s3_get_object(&context);
}

fn do_s3_get_object(context: &impl Context) {
    // subsegment will have the name "S3" and `aws.operation` "GetObject"
    let subsegment = context.enter_subsegment(AwsNamespace::new("S3", "GetObject"));

    // call S3 GetObject ...

    // if you are using `aws-sdk-s3` crate, you can update the subsegment
    // with the request ID. suppose `out` is the output of the `GetObject`
    // operation:
    //
    //     subsegment
    //         .namespace_mut()
    //         .zip(out.request_id())
    //         .map(|(ns, id)| ns.request_id(id));

    // the subsegment will be ended and reported when it is dropped
}
```

### Subsegment of a remote service call

Here is an example to record a subsegment of a remote service call within a Lambda function invocation instrumented with AWS X-Ray:

```rust
use xray_lite::{Client, Context, RemoteNamespace, SubsegmentContext};

fn main() {
   // reads AWS_XRAY_DAEMON_ADDRESS
   let client = Client::from_lambda_env().unwrap();
   // reads _X_AMZN_TRACE_ID
   let context = SubsegmentContext::from_lambda_env(client).unwrap();

   do_some_request(&context);
}

fn do_some_request(context: &impl Context) {
    // subsegment will have the name "readme example",
    // `http.request.method` "POST", and `http.request.url` "https://codemonger.io/"
    let subsegment = context.enter_subsegment(RemoteNamespace::new(
        "readme example",
        "GET",
        "https://codemonger.io/",
    ));

    // do some request ...

    // the subsegment will be ended and reported when it is dropped
}
```

### Custom subsegment

Here is an example to record a custom subsegment within a Lambda function invocation instrumented with AWS X-Ray:

```rust
use xray_lite::{Client, Context, CustomNamespace, SubsegmentContext};

fn main() {
   // reads AWS_XRAY_DAEMON_ADDRESS
   let client = Client::from_lambda_env().unwrap();
   // reads _X_AMZN_TRACE_ID
   let context = SubsegmentContext::from_lambda_env(client).unwrap()
       .with_name_prefix("readme_example.");

   do_something(&context);
}

fn do_something(context: &impl Context) {
    // subsegment will have the name "readme_example.do_something"
    let subsegment = context.enter_subsegment(CustomNamespace::new("do_something"));

    // do some thing ...

    // the subsegment will be ended and reported when it is dropped
}
```

## API Documentation

<https://codemonger-io.github.io/xray-lite/api/xray_lite/>

## Acknowledgements

This project is built on top of the [great work](https://github.com/softprops/xray) of [Doug Tangren (softprops)](https://github.com/softprops).