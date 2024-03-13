//! Namespace encapsulation for subsegments.

use crate::segment::{AwsOperation, Http, Request, Response, Subsegment};

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
    response_status: Option<u16>,
}

impl AwsNamespace {
    /// Creates a namespace for an AWS service operation.
    pub fn new(service: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            operation: operation.into(),
            request_id: None,
            response_status: None,
        }
    }

    /// Sets the request ID.
    pub fn request_id(&mut self, request_id: impl Into<String>) -> &mut Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Sets the response status.
    pub fn response_status(&mut self, status: u16) -> &mut Self {
        self.response_status = Some(status);
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
        if let Some(response_status) = self.response_status {
            if let Some(http) = subsegment.http.as_mut() {
                if let Some(response) = http.response.as_mut() {
                    if response.status.is_none() {
                        response.status = Some(response_status);
                    }
                } else {
                    http.response = Some(Response {
                        status: Some(response_status),
                        ..Response::default()
                    });
                }
            } else {
                subsegment.http = Some(Http {
                    response: Some(Response {
                        status: Some(response_status),
                        ..Response::default()
                    }),
                    ..Http::default()
                });
            }
        }
    }
}

/// Namespace for an arbitrary remote service.
#[derive(Debug)]
pub struct RemoteNamespace {
    name: String,
    method: String,
    url: String,
    response_status: Option<u16>,
}

impl RemoteNamespace {
    /// Creates a namespace for a remote service.
    pub fn new(name: impl Into<String>, method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            method: method.into(),
            url: url.into(),
            response_status: None,
        }
    }

    /// Sets the response status.
    pub fn response_status(&mut self, status: u16) -> &mut Self {
        self.response_status = Some(status);
        self
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
        if let Some(response_status) = self.response_status {
            let http = subsegment.http.as_mut().expect("http must have been set");
            if let Some(response) = http.response.as_mut() {
                if response.status.is_none() {
                    response.status = Some(response_status);
                }
            } else {
                http.response = Some(Response {
                    status: Some(response_status),
                    ..Response::default()
                });
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_namespace_should_have_service_name_as_name() {
        let namespace = AwsNamespace::new("S3", "GetObject");
        assert_eq!(namespace.name(""), "S3");
        assert_eq!(namespace.name("prefix"), "S3");
    }

    #[test]
    fn aws_namespace_should_update_subsegment_with_aws_operation() {
        let namespace = AwsNamespace::new("S3", "GetObject");
        let mut subsegment = Subsegment::default();
        namespace.update_subsegment(&mut subsegment);
        assert_eq!(subsegment.namespace.unwrap(), "aws");
        assert_eq!(subsegment.aws.unwrap().operation.unwrap(), "GetObject");
    }

    #[test]
    fn aws_namespace_should_update_subsegment_with_request_id() {
        let mut namespace = AwsNamespace::new("S3", "GetObject");
        namespace.request_id("12345");
        let mut subsegment = Subsegment::default();
        namespace.update_subsegment(&mut subsegment);
        assert_eq!(subsegment.aws.unwrap().request_id.unwrap(), "12345");
    }

    #[test]
    fn aws_namespace_should_update_subsegment_with_response_status() {
        let mut namespace = AwsNamespace::new("S3", "GetObject");
        namespace.response_status(200);
        let mut subsegment = Subsegment::default();
        namespace.update_subsegment(&mut subsegment);
        assert_eq!(
            subsegment
                .http
                .expect("http")
                .response
                .expect("response")
                .status
                .expect("status"),
            200,
        );
    }

    #[test]
    fn remote_namespace_should_have_name_as_name() {
        let namespace = RemoteNamespace::new("codemonger.io", "GET", "https://codemonger.io/");
        assert_eq!(namespace.name(""), "codemonger.io");
        assert_eq!(namespace.name("prefix"), "codemonger.io");
    }

    #[test]
    fn remote_namespace_should_update_subsegment_with_remote_service() {
        let namespace = RemoteNamespace::new("codemonger.io", "GET", "https://codemonger.io/");
        let mut subsegment = Subsegment::default();
        namespace.update_subsegment(&mut subsegment);
        assert_eq!(subsegment.namespace.unwrap(), "remote");
        let request = subsegment.http.expect("http").request.expect("request");
        assert_eq!(request.method.unwrap(), "GET");
        assert_eq!(request.url.unwrap(), "https://codemonger.io/");
    }

    #[test]
    fn remote_namespace_should_update_subsegment_with_response_status() {
        let mut namespace = RemoteNamespace::new("codemonger.io", "GET", "https://codemonger.io/");
        namespace.response_status(200);
        let mut subsegment = Subsegment::default();
        namespace.update_subsegment(&mut subsegment);
        assert_eq!(
            subsegment
                .http
                .expect("http")
                .response
                .expect("response")
                .status
                .expect("status"),
            200,
        );
    }

    #[test]
    fn custom_namespace_should_have_prefixed_name() {
        let namespace = CustomNamespace::new("TestSubsegment");
        assert_eq!(namespace.name(""), "TestSubsegment");
        assert_eq!(namespace.name("prefix"), "prefixTestSubsegment");
    }
}
