use crate::{Error, Header, Result};

pub(crate) fn header() -> Result<Header> {
    std::env::var("_X_AMZN_TRACE_ID")
        .map_err(|_| Error::MissingEnvVar("_X_AMZN_TRACE_ID"))?
        .parse::<Header>()
        .map_err(|e| Error::BadConfig(format!("invalid X-Ray trace ID header value: {e}")))
}
