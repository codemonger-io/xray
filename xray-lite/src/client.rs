//! X-Ray daemon client.

use std::env;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use serde::Serialize;

use crate::error::{Error, Result};

/// X-Ray client interface.
pub trait Client: Clone + std::fmt::Debug + Send + Sync {
    /// Sends a segment to the xray daemon this client is connected to.
    fn send<S>(&self, data: &S) -> Result<()>
    where
        S: Serialize;
}

/// X-Ray daemon client.
#[derive(Clone, Debug)]
pub struct DaemonClient {
    socket: Arc<UdpSocket>,
}

impl DaemonClient {
    const HEADER: &'static [u8] = br#"{"format": "json", "version": 1}"#;
    const DELIMITER: &'static [u8] = &[b'\n'];

    /// Return a new X-Ray client connected
    /// to the provided `addr`
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let socket = Arc::new(UdpSocket::bind(&[([0, 0, 0, 0], 0).into()][..])?);
        socket.set_nonblocking(true)?;
        socket.connect(addr)?;
        Ok(DaemonClient { socket })
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
            .map_err(|_| Error::MissingEnvVar("AWS_XRAY_DAEMON_ADDRESS"))?
            .parse::<SocketAddr>()
            .map_err(|e| Error::BadConfig(format!("invalid X-Ray daemon address: {e}")))?;
        DaemonClient::new(addr)
    }

    #[inline]
    fn packet<S>(data: S) -> Result<Vec<u8>>
    where
        S: Serialize,
    {
        let bytes = serde_json::to_vec(&data)?;
        Ok([Self::HEADER, Self::DELIMITER, &bytes].concat())
    }
}

impl Client for DaemonClient {
    fn send<S>(&self, data: &S) -> Result<()>
    where
        S: Serialize,
    {
        self.socket.send(&Self::packet(data)?)?;
        Ok(())
    }
}

/// Infallible client.
#[derive(Clone, Debug)]
pub enum InfallibleClient<C> {
    /// Operational client.
    Op(C),
    /// Non-operational client.
    Noop,
}

impl<C> InfallibleClient<C> {
    /// Creates a new infallible client from a result of client creation.
    pub fn new<E>(result: std::result::Result<C, E>) -> Self {
        match result {
            Ok(client) => Self::Op(client),
            Err(_) => Self::Noop,
        }
    }
}

impl<C> Client for InfallibleClient<C>
where
    C: Client,
{
    fn send<S>(&self, data: &S) -> Result<()>
    where
        S: Serialize,
    {
        match self {
            Self::Op(client) => client.send(data),
            Self::Noop => Ok(()),
        }
    }
}

/// Conversion into an [`InfallibleClient`].
///
/// This is useful if you want to fall back to a "no-op" client if the creation
/// of a client fails.
///
/// ```
/// use xray_lite::{
///     Context as _,
///     CustomNamespace,
///     DaemonClient,
///     IntoInfallibleClient as _,
///     SubsegmentContext,
/// };
///
/// fn main() {
///     let client = DaemonClient::from_lambda_env().into_infallible();
///     # std::env::set_var("_X_AMZN_TRACE_ID", "Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1");
///     let context = SubsegmentContext::from_lambda_env(client).unwrap();
///     let _session = context.enter_subsegment(CustomNamespace::new("readme.example"));
/// }
/// ```
pub trait IntoInfallibleClient {
    /// Client type.
    type Client: Client;

    /// Converts a value into an [`InfallibleClient`].
    fn into_infallible(self) -> InfallibleClient<Self::Client>;
}

impl<C> IntoInfallibleClient for std::result::Result<C, Error>
where
    C: Client,
{
    type Client = C;

    fn into_infallible(self) -> InfallibleClient<C> {
        InfallibleClient::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_prefixes_packets_with_header() {
        assert_eq!(
            DaemonClient::packet(serde_json::json!({
                "foo": "bar"
            }))
            .unwrap(),
            [
                br#"{"format": "json", "version": 1}"# as &[u8],
                &[b'\n'],
                br#"{"foo":"bar"}"#,
            ]
            .concat()
        )
    }
}
