//! Subsegment session management.

use crate::client::Client;
use crate::header::Header;
use crate::namespace::Namespace;
use crate::segment::Subsegment;

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
    pub(crate) fn new(client: Client, header: &Header, namespace: T, name_prefix: &str) -> Self {
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
