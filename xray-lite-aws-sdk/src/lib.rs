#![warn(missing_docs)]

//! Extension of `xray-lite` for [AWS SDK for Rust](https://aws.amazon.com/sdk-for-rust/).

use std::sync::{Arc, Mutex};

use aws_smithy_runtime_api::box_error::BoxError;
use aws_smithy_runtime_api::client::interceptors::context::{
    BeforeTransmitInterceptorContextMut, BeforeTransmitInterceptorContextRef,
    FinalizerInterceptorContextRef,
};
use aws_smithy_runtime_api::client::interceptors::Intercept;
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
use aws_smithy_types::config_bag::ConfigBag;
use aws_types::request_id::RequestId;

use xray_lite::{AwsNamespace, Context, Header, SubsegmentSession};

/// Extension of [`Context`].
///
/// This trait is automatically implemented for any [`Context`] that satisfies
/// the bounds.
pub trait ContextExt: Context + Clone + std::fmt::Debug + Send + Sync + 'static {
    /// Creates an [`Intercept`](https://docs.rs/aws-smithy-runtime-api/1.1.7/aws_smithy_runtime_api/client/interceptors/trait.Intercept.html)
    /// for the AWS service request.
    ///
    /// A returned `Intercept` implements the following hooks:
    /// 1. [`read_before_attempt`](https://docs.rs/aws-smithy-runtime-api/1.1.7/aws_smithy_runtime_api/client/interceptors/trait.Intercept.html#method.read_before_attempt):
    ///    Starts a subsegment of the AWS service request
    /// 2. [`modify_before_transmit`](https://docs.rs/aws-smithy-runtime-api/1.1.7/aws_smithy_runtime_api/client/interceptors/trait.Intercept.html#method.modify_before_transmit):
    ///    Injects the `X-Amzn-Trace-Id` header into the request
    /// 3. [`read_after_attempt`](https://docs.rs/aws-smithy-runtime-api/1.1.7/aws_smithy_runtime_api/client/interceptors/trait.Intercept.html#method.read_after_attempt):
    ///    Updates the subsegment with the request ID and the response status,
    ///    and reports the subsegment to the X-Ray daemon
    fn intercept_operation(
        &self,
        service: impl Into<String>,
        operation: impl Into<String>,
    ) -> impl Intercept + 'static {
        XrayIntercept::new_with_operation(self.clone(), service, operation)
    }
}

impl<T> ContextExt for T where T: Context + Clone + std::fmt::Debug + Send + Sync + 'static {}

#[derive(Debug)]
struct XrayIntercept<T>
where
    T: Context + Clone + std::fmt::Debug + Send + Sync + 'static,
{
    context: T,
    service: String,
    operation: String,
    // session is unnecessarily wrapped in Mutex because `Intercept` is
    // immutable during its method calls.
    session: Arc<Mutex<Option<SubsegmentSession<AwsNamespace>>>>,
}

impl<T> XrayIntercept<T>
where
    T: Context + Clone + std::fmt::Debug + Send + Sync + 'static,
{
    fn new_with_operation(
        context: T,
        service: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            context,
            service: service.into(),
            operation: operation.into(),
            session: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T> Intercept for XrayIntercept<T>
where
    T: Context + Clone + std::fmt::Debug + Send + Sync + 'static,
{
    fn name(&self) -> &'static str {
        "XrayIntercept"
    }

    fn read_before_attempt(
        &self,
        _context: &BeforeTransmitInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        let session = self.context.enter_subsegment(AwsNamespace::new(
            self.service.clone(),
            self.operation.clone(),
        ));
        *self.session.lock().unwrap() = Some(session);
        Ok(())
    }

    fn modify_before_transmit(
        &self,
        context: &mut BeforeTransmitInterceptorContextMut<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        let trace_id = self
            .session
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.x_amzn_trace_id());
        if let Some(trace_id) = trace_id {
            context
                .request_mut()
                .headers_mut()
                .insert(Header::NAME, trace_id);
        }
        Ok(())
    }

    fn read_after_attempt(
        &self,
        context: &FinalizerInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        let mut session = self.session.lock().unwrap();
        if let Some(mut session) = session.take() {
            if let Some(namespace) = session.namespace_mut() {
                if let Some(response) = context.response() {
                    namespace.response_status(response.status().as_u16());
                    if let Some(request_id) = response.request_id() {
                        namespace.request_id(request_id);
                    }
                }
            }
        }
        Ok(())
    }
}
