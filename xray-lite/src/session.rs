//! Subsegment session management.

use crate::client::Client;
use crate::header::Header;
use crate::namespace::Namespace;
use crate::segment::Subsegment;

/// Subsegment session.
#[derive(Debug)]
pub enum SubsegmentSession<C, N>
where
    C: Client,
    N: Namespace + Send + Sync,
{
    /// Entered subsegment.
    Entered {
        /// X-Ray client.
        client: C,
        /// X-Amzn-Trace-Id header.
        header: Header,
        /// Subsegment.
        subsegment: Subsegment,
        /// Namespace.
        namespace: N,
    },
    /// Failed subsegment.
    Failed,
}

impl<C, N> SubsegmentSession<C, N>
where
    C: Client,
    N: Namespace + Send + Sync,
{
    pub(crate) fn new(client: C, header: &Header, namespace: N, name_prefix: &str) -> Self {
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

    pub(crate) fn failed() -> Self {
        Self::Failed
    }

    /// Returns the `x-amzn-trace-id` header value.
    pub fn x_amzn_trace_id(&self) -> Option<String> {
        match self {
            Self::Entered { header, .. } => Some(header.to_string()),
            Self::Failed => None,
        }
    }

    /// Returns the namespace as a mutable reference.
    pub fn namespace_mut(&mut self) -> Option<&mut N> {
        match self {
            Self::Entered { namespace, .. } => Some(namespace),
            Self::Failed => None,
        }
    }
}

impl<C, N> Drop for SubsegmentSession<C, N>
where
    C: Client,
    N: Namespace + Send + Sync,
{
    fn drop(&mut self) {
        match self {
            Self::Entered {
                client,
                subsegment,
                namespace,
                ..
            } => {
                subsegment.end();
                namespace.update_subsegment(subsegment);
                let _ = client
                    .send(subsegment)
                    .map_err(|e| eprintln!("failed to end subsegment: {e}"));
            }
            Self::Failed => (),
        }
    }
}
