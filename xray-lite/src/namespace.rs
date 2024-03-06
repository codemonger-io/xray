//! Namespace encapsulation for subsegments.

use crate::segment::{AwsOperation, Http, Request, Subsegment};

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

/// Namespace for an arbitrary remote service.
#[derive(Debug)]
pub struct RemoteNamespace {
    name: String,
    method: String,
    url: String,
}

impl RemoteNamespace {
    /// Creates a namespace for a remote service.
    pub fn new(name: impl Into<String>, method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            method: method.into(),
            url: url.into(),
        }
    }
}

impl Namespace for RemoteNamespace {
    fn name(&self, _prefix: &str) -> String {
        self.name.clone()
    }

    fn update_subsegment(&self, subsegment: &mut Subsegment) {
        if subsegment.namespace.is_none() {
            subsegment.namespace = Some("remote".to_string());
        }
        if let Some(http) = subsegment.http.as_mut() {
            if let Some(request) = http.request.as_mut() {
                if request.method.is_none() {
                    request.method = Some(self.method.clone());
                }
                if request.url.is_none() {
                    request.url = Some(self.url.clone());
                }
            } else {
                http.request = Some(Request {
                    url: Some(self.url.clone()),
                    method: Some(self.method.clone()),
                    ..Request::default()
                });
            }
        } else {
            subsegment.http = Some(Http {
                request: Some(Request {
                    url: Some(self.url.clone()),
                    method: Some(self.method.clone()),
                    ..Request::default()
                }),
                ..Http::default()
            });
        }
    }
}

/// Namespace for a custom subsegment.
#[derive(Debug)]
pub struct CustomNamespace {
    name: String,
}

impl CustomNamespace {
    /// Creates a namespace for a custom subsegment.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Namespace for CustomNamespace {
    fn name(&self, prefix: &str) -> String {
        format!("{}{}", prefix, self.name)
    }

    // does nothing
    fn update_subsegment(&self, _subsegment: &mut Subsegment) {}
}
