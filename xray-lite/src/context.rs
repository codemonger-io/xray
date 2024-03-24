//! Tracing context.

use crate::client::Client;
use crate::error::Result;
use crate::header::Header;
use crate::lambda;
use crate::namespace::Namespace;
use crate::session::SubsegmentSession;

/// Context.
pub trait Context {
    /// Client type.
    type Client: Client;

    /// Enters in a new subsegment.
    ///
    /// [`SubsegmentSession`] records the end of the subsegment when it is
    /// dropped.
    fn enter_subsegment<T>(&self, namespace: T) -> SubsegmentSession<Self::Client, T>
    where
        T: Namespace + Send + Sync;
}

/// Context as a subsegment of an existing segment.
#[derive(Clone, Debug)]
pub struct SubsegmentContext<C> {
    client: C,
    header: Header,
    name_prefix: String,
}

impl<C> SubsegmentContext<C> {
    /// Creates a new context from the Lambda environment variable.
    ///
    /// The following environment variable must be set:
    /// - `_X_AMZN_TRACE_ID`: AWS X-Ray trace ID
    ///
    /// Please refer to the [AWS documentation](https://docs.aws.amazon.com/lambda/latest/dg/configuration-envvars.html#configuration-envvars-runtime)
    /// for more details.
    pub fn from_lambda_env(client: C) -> Result<Self> {
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
    /// Only subsegments associated with
    /// [`CustomNamespace`][crate::namespace::CustomNamespace] are affected.
    pub fn with_name_prefix(self, prefix: impl Into<String>) -> Self {
        Self {
            client: self.client,
            header: self.header,
            name_prefix: prefix.into(),
        }
    }
}

impl<C> Context for SubsegmentContext<C>
where
    C: Client,
{
    type Client = C;

    fn enter_subsegment<T>(&self, namespace: T) -> SubsegmentSession<Self::Client, T>
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

/// Infallible context.
///
/// This context is useful if you want to fall back to "no-op" when creation of
/// the underlying context fails.
pub enum InfallibleContext<T> {
    /// Operational context.
    Op(T),
    /// Non-operational context.
    Noop,
}

impl<T> InfallibleContext<T>
where
    T: Context,
{
    /// Constructs from a result of the underlying context creation.
    pub fn new<E>(result: std::result::Result<T, E>) -> Self {
        match result {
            Ok(context) => Self::Op(context),
            Err(_) => Self::Noop,
        }
    }
}

impl<T> Context for InfallibleContext<T>
where
    T: Context,
{
    type Client = T::Client;

    fn enter_subsegment<U>(&self, namespace: U) -> SubsegmentSession<Self::Client, U>
    where
        U: Namespace + Send + Sync,
    {
        match self {
            Self::Op(context) => context.enter_subsegment(namespace),
            Self::Noop => SubsegmentSession::failed(),
        }
    }
}

impl<T> Clone for InfallibleContext<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Op(context) => Self::Op(context.clone()),
            Self::Noop => Self::Noop,
        }
    }
}

impl<T> std::fmt::Debug for InfallibleContext<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Op(context) => write!(f, "InfallibleContext::Op({:?})", context),
            Self::Noop => write!(f, "InfallibleContext::Noop"),
        }
    }
}

/// Conversion into an infallible context.
///
/// You can convert `Result<Context, _>` into an infallible context by using
/// this trait.`
///
/// ```
/// use xray_lite::{
///     Context as _,
///     DaemonClient,
///     IntoInfallibleContext as _,
///     CustomNamespace,
///     SubsegmentContext,
/// };
///
/// fn main() {
///     # std::env::set_var("AWS_XRAY_DAEMON_ADDRESS", "127.0.0.1:2000");
///     let client = DaemonClient::from_lambda_env().unwrap();
///     let context = SubsegmentContext::from_lambda_env(client).into_infallible();
///     let _session = context.enter_subsegment(CustomNamespace::new("readme.example"));
/// }
/// ```
pub trait IntoInfallibleContext {
    /// Underlying context type.
    type Context: Context;

    /// Converts into an infallible context.
    fn into_infallible(self) -> InfallibleContext<Self::Context>;
}

impl<T, E> IntoInfallibleContext for std::result::Result<T, E>
where
    T: Context,
{
    type Context = T;

    fn into_infallible(self) -> InfallibleContext<Self::Context> {
        InfallibleContext::new(self)
    }
}
