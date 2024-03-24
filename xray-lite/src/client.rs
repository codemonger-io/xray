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
