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
//! use xray::{AwsNamespace, Client, Context as _, SubsegmentContext};
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
//! fn do_s3_get_object(context: &SubsegmentContext) {
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
//!     //         .map(|n, id| n.request_id(id));
//!
//!     // the subsegment will be ended and reported when it is dropped
//! }
//! ```

use serde::Serialize;
use std::{
    env,
    net::{SocketAddr, UdpSocket},
    result::Result as StdResult,
    sync::Arc,
};

mod epoch;
mod error;
mod header;
mod hexbytes;
mod lambda;
mod segment;
mod segment_id;
mod trace_id;

pub use crate::{
    epoch::Seconds, error::Error, header::Header, segment::*, segment_id::SegmentId,
    trace_id::TraceId,
};

/// Type alias for Results which may return `xray::Errors`
pub type Result<T> = StdResult<T, Error>;

/// X-Ray daemon client interface
#[derive(Clone, Debug)]
pub struct Client {
    socket: Arc<UdpSocket>,
}

impl Client {
    const HEADER: &'static [u8] = br#"{"format": "json", "version": 1}"#;
    const DELIMITER: &'static [u8] = &[b'\n'];

    /// Return a new X-Ray client connected
    /// to the provided `addr`
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let socket = Arc::new(UdpSocket::bind(&[([0, 0, 0, 0], 0).into()][..])?);
        socket.set_nonblocking(true)?;
        socket.connect(&addr)?;
        Ok(Client { socket })
    }

    /// Creates a new X-Ray client from the Lambda environment variable.
    ///
    /// The following environment variable must be set:
    /// - `AWS_XRAY_DAEMON_ADDRESS`: X-Ray daemon address
    ///
    /// Please refer to the [AWS documentation](https://docs.aws.amazon.com/lambda/latest/dg/configuration-envvars.html#configuration-envvars-runtime)
    /// for more details.
    pub fn from_lambda_env() -> Result<Self> {
        let addr: SocketAddr = env::var("AWS_XRAY_DAEMON_ADDRESS")
            .map_err(|_| Error::MissingEnvVar(&"AWS_XRAY_DAEMON_ADDRESS"))?
            .parse::<SocketAddr>()
            .map_err(|e| Error::BadConfig(format!("invalid X-Ray daemon address: {e}")))?;
        Client::new(addr)
    }

    #[inline]
    fn packet<S>(data: S) -> Result<Vec<u8>>
    where
        S: Serialize,
    {
        let bytes = serde_json::to_vec(&data)?;
        Ok([Self::HEADER, Self::DELIMITER, &bytes].concat())
    }

    /// send a segment to the xray daemon this client is connected to
    pub fn send<S>(
        &self,
        data: &S,
    ) -> Result<()>
    where
        S: Serialize,
    {
        self.socket.send(&Self::packet(data)?)?;
        Ok(())
    }
}

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

/// Subsegment session.
pub enum SubsegmentSession<T>
where
    T: Namespace + Send + Sync,
{
    /// Entered subsegment.
    Entered {
        /// X-Ray client.
        client: Client,
        /// X-Amzn-Trace-Id header.
        header: Header,
        /// Subsegment.
        subsegment: Subsegment,
        /// Namespace.
        namespace: T,
    },
    /// Failed subsegment.
    Failed,
}

impl<T> SubsegmentSession<T>
where
    T: Namespace + Send + Sync,
{
    fn new(client: Client, header: &Header, namespace: T, name_prefix: &str) -> Self {
        let mut subsegment = Subsegment::begin(
            header.trace_id.clone(),
            header.parent_id.clone(),
            namespace.name(name_prefix),
        );
        namespace.update_subsegment(&mut subsegment);
        match client.send(&subsegment) {
            Ok(_) => Self::Entered {
                client,
                header: header.with_parent_id(subsegment.id.clone()),
                subsegment,
                namespace,
            },
            Err(_) => Self::Failed,
        }
    }

    /// Returns the `x-amzn-trace-id` header value.
    pub fn x_amzn_trace_id(&self) -> Option<String> {
        match self {
            Self::Entered { header, .. } => Some(header.to_string()),
            Self::Failed => None,
        }
    }

    /// Returns the namespace as a mutable reference.
    pub fn namespace_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Entered { namespace, .. } => Some(namespace),
            Self::Failed => None,
        }
    }
}

impl<T> Drop for SubsegmentSession<T>
where
    T: Namespace + Send + Sync,
{
    fn drop(&mut self) {
        match self {
            Self::Entered { client, subsegment, namespace, .. } => {
                subsegment.end();
                namespace.update_subsegment(subsegment);
                let _ = client.send(subsegment)
                    .map_err(|e| eprintln!("failed to end subsegment: {e}"));
            }
            Self::Failed => (),
        }
    }
}

/// Namespace.
pub trait Namespace {
    /// Name of the namespace.
    ///
    /// `prefix` may be ignored.
    fn name(&self, prefix: &str) -> String;

    /// Updates the subsegment.
    fn update_subsegment(&self, subsegment: &mut Subsegment);
}

/// Namespace for an AWS service.
#[derive(Debug)]
pub struct AwsNamespace {
    service: String,
    operation: String,
    request_id: Option<String>,
}

impl AwsNamespace {
    /// Creates a namespace for an AWS service operation.
    pub fn new(service: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            operation: operation.into(),
            request_id: None,
        }
    }

    /// Sets the request ID.
    pub fn request_id(&mut self, request_id: impl Into<String>) -> &mut Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl Namespace for AwsNamespace {
    fn name(&self, _prefix: &str) -> String {
        self.service.clone()
    }

    fn update_subsegment(&self, subsegment: &mut Subsegment) {
        if subsegment.namespace.is_none() {
            subsegment.namespace = Some("aws".to_string());
        }
        if let Some(aws) = subsegment.aws.as_mut() {
            if aws.operation.is_none() {
                aws.operation = Some(self.operation.clone());
            }
            if aws.request_id.is_none() {
                aws.request_id = self.request_id.clone();
            }
        } else {
            subsegment.aws = Some(AwsOperation {
                operation: Some(self.operation.clone()),
                request_id: self.request_id.clone(),
                ..AwsOperation::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_prefixes_packets_with_header() {
        assert_eq!(
            Client::packet(serde_json::json!({
                "foo": "bar"
            }))
            .unwrap(),
            [
                br#"{"format": "json", "version": 1}"# as &[u8],
                &[b'\n'],
                br#"{"foo":"bar"}"#,
            ].concat()
        )
    }
}
