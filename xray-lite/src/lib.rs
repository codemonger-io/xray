#![warn(missing_docs)]
//#![deny(warnings)]
//! Provides a client interface for [AWS X-Ray](https://aws.amazon.com/xray/)
//!
//! ### Examples
//!
//! #### Subsegment of AWS service operation
//!
//! Here is an example to record a subsegment of an AWS service operation
//! within a Lambda function invocation instrumented with AWS X-Ray:
//!
//! ```
//! use xray_lite::{AwsNamespace, Client, Context, SubsegmentContext};
//!
//! fn main() {
//!    // reads AWS_XRAY_DAEMON_ADDRESS
//!    # std::env::set_var("AWS_XRAY_DAEMON_ADDRESS", "127.0.0.1:2000");
//!    let client = Client::from_lambda_env().unwrap();
//!    // reads _X_AMZN_TRACE_ID
//!    # std::env::set_var("_X_AMZN_TRACE_ID", "Root=1-65dfb5a1-0123456789abcdef01234567;Parent=0123456789abcdef;Sampled=1");
//!    let context = SubsegmentContext::from_lambda_env(client).unwrap();
//!
//!    do_s3_get_object(&context);
//! }
//!
//! fn do_s3_get_object(context: &impl Context) {
//!     // subsegment will have the name "S3" and `aws.operation` "GetObject"
//!     let subsegment = context.enter_subsegment(AwsNamespace::new("S3", "GetObject"));
//!
//!     // call S3 GetObject ...
//!
//!     // if you are using `aws-sdk-s3` crate, you can update the subsegment
//!     // with the request ID. suppose `out` is the output of the `GetObject`
//!     // operation:
//!     //
//!     //     subsegment
//!     //         .namespace_mut()
//!     //         .zip(out.request_id())
//!     //         .map(|(ns, id)| ns.request_id(id));
//!
//!     // the subsegment will be ended and reported when it is dropped
//! }
//! ```
//!
//! #### Subsegment of remote service call
//!
//! Here is an example to record a subsegment of a remote service call within a
//! Lambda function invocation intstrumented with AWS X-Ray:
//!
//! ```
//! use xray_lite::{Client, Context, RemoteNamespace, SubsegmentContext};
//!
//! fn main() {
//!    // reads AWS_XRAY_DAEMON_ADDRESS
//!    # std::env::set_var("AWS_XRAY_DAEMON_ADDRESS", "127.0.0.1:2000");
//!    let client = Client::from_lambda_env().unwrap();
//!    // reads _X_AMZN_TRACE_ID
//!    # std::env::set_var("_X_AMZN_TRACE_ID", "Root=1-65dfb5a1-0123456789abcdef01234567;Parent=0123456789abcdef;Sampled=1");
//!    let context = SubsegmentContext::from_lambda_env(client).unwrap();
//!
//!    do_some_request(&context);
//! }
//!
//! fn do_some_request(context: &impl Context) {
//!     // subsegment will have the name "readme example",
//!     // `http.request.method` "POST", and `http.request.url` "https://codemonger.io/"
//!     let subsegment = context.enter_subsegment(RemoteNamespace::new(
//!         "readme example",
//!         "GET",
//!         "https://codemonger.io/",
//!     ));
//!
//!     // do some request ...
//!
//!     // the subsegment will be ended and reported when it is dropped
//! }
//! ```
//!
//! #### Custom subsegment
//!
//! Here is an example to record a custom subsegment within a Lambda function
//! invocation intstrumented with AWS X-Ray:
//!
//! ```
//! use xray_lite::{Client, Context, CustomNamespace, SubsegmentContext};
//!
//! fn main() {
//!    // reads AWS_XRAY_DAEMON_ADDRESS
//!    # std::env::set_var("AWS_XRAY_DAEMON_ADDRESS", "127.0.0.1:2000");
//!    let client = Client::from_lambda_env().unwrap();
//!    // reads _X_AMZN_TRACE_ID
//!    # std::env::set_var("_X_AMZN_TRACE_ID", "Root=1-65dfb5a1-0123456789abcdef01234567;Parent=0123456789abcdef;Sampled=1");
//!    let context = SubsegmentContext::from_lambda_env(client).unwrap()
//!        .with_name_prefix("readme_example.");
//!
//!    do_something(&context);
//! }
//!
//! fn do_something(context: &impl Context) {
//!     // subsegment will have the name "readme_example.do_something"
//!     let subsegment = context.enter_subsegment(CustomNamespace::new("do_something"));
//!
//!     // do some thing ...
//!
//!     // the subsegment will be ended and reported when it is dropped
//! }
//! ```

mod client;
mod epoch;
mod error;
mod header;
mod hexbytes;
mod lambda;
mod namespace;
mod segment;
mod segment_id;
mod session;
mod trace_id;

pub use crate::{
    client::Client,
    epoch::Seconds,
    error::{Error, Result},
    header::Header,
    namespace::{AwsNamespace, CustomNamespace, Namespace, RemoteNamespace},
    segment::*,
    segment_id::SegmentId,
    session::SubsegmentSession,
    trace_id::TraceId,
};

/// Context.
pub trait Context {
    /// Enters in a new subsegment.
    ///
    /// [`SubsegmentSession`] records the end of the subsegment when it is
    /// dropped.
    fn enter_subsegment<T>(&self, namespace: T) -> SubsegmentSession<T>
    where
        T: Namespace + Send + Sync;
}

/// Context as a subsegment of an existing segment.
#[derive(Debug)]
pub struct SubsegmentContext {
    client: Client,
    header: Header,
    name_prefix: String,
}

impl SubsegmentContext {
    /// Creates a new context from the Lambda environment variable.
    ///
    /// The following environment variable must be set:
    /// - `_X_AMZN_TRACE_ID`: AWS X-Ray trace ID
    ///
    /// Please refer to the [AWS documentation](https://docs.aws.amazon.com/lambda/latest/dg/configuration-envvars.html#configuration-envvars-runtime)
    /// for more details.
    pub fn from_lambda_env(client: Client) -> Result<Self> {
        let header = lambda::header()?;
        Ok(Self {
            client,
            header,
            name_prefix: "".to_string(),
        })
    }

    /// Updates the context with a given name prefix.
    ///
    /// The name prefix is prepended to the name of every custom subsegment.
    /// Only subsegments associated with [`CustomNamespace`] are affected.
    pub fn with_name_prefix(self, prefix: impl Into<String>) -> Self {
        Self {
            client: self.client,
            header: self.header,
            name_prefix: prefix.into(),
        }
    }
}

impl Context for SubsegmentContext {
    fn enter_subsegment<T>(&self, namespace: T) -> SubsegmentSession<T>
    where
        T: Namespace + Send + Sync,
    {
        SubsegmentSession::new(
            self.client.clone(),
            &self.header,
            namespace,
            &self.name_prefix,
        )
    }
}
