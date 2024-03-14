//! X-Ray [tracing header](https://docs.aws.amazon.com/xray/latest/devguide/xray-concepts.html?shortFooter=true#xray-concepts-tracingheader)
//! parser

use crate::{SegmentId, TraceId};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
};

#[derive(PartialEq, Clone, Copy, Debug, Default)]
pub enum SamplingDecision {
    /// Sampled indicates the current segment has been
    /// sampled and will be sent to the X-Ray daemon.
    Sampled,
    /// NotSampled indicates the current segment has
    /// not been sampled.
    NotSampled,
    ///sampling decision will be
    /// made by the downstream service and propagated
    /// back upstream in the response.
    Requested,
    /// Unknown indicates no sampling decision will be made.
    #[default]
    Unknown,
}

impl<'a> From<&'a str> for SamplingDecision {
    fn from(value: &'a str) -> Self {
        match value {
            "Sampled=1" => SamplingDecision::Sampled,
            "Sampled=0" => SamplingDecision::NotSampled,
            "Sampled=?" => SamplingDecision::Requested,
            _ => SamplingDecision::Unknown,
        }
    }
}

impl Display for SamplingDecision {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SamplingDecision::Sampled => "Sampled=1",
                SamplingDecision::NotSampled => "Sampled=0",
                SamplingDecision::Requested => "Sampled=?",
                _ => "",
            }
        )?;
        Ok(())
    }
}

/// Parsed representation of `X-Amzn-Trace-Id` request header
#[derive(PartialEq, Clone, Debug, Default)]
pub struct Header {
    pub(crate) trace_id: TraceId,
    pub(crate) parent_id: Option<SegmentId>,
    pub(crate) sampling_decision: SamplingDecision,
    additional_data: HashMap<String, String>,
}

impl Header {
    /// HTTP header name associated with X-Ray trace data
    ///
    /// HTTP header values should be the Display serialization of Header structs
    pub const NAME: &'static str = "X-Amzn-Trace-Id";

    /// Creates a new Header with a given trace ID.
    pub fn new(trace_id: TraceId) -> Self {
        Header {
            trace_id,
            ..Header::default()
        }
    }

    /// Creates a new Header with the parent ID replaced.
    pub fn with_parent_id(&self, parent_id: SegmentId) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            parent_id: Some(parent_id),
            sampling_decision: self.sampling_decision,
            additional_data: self.additional_data.clone(),
        }
    }

    /// Creates a new Header with the sampling decision replaced.
    pub fn with_sampling_decision(&self, decision: SamplingDecision) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            parent_id: self.parent_id.clone(),
            sampling_decision: decision,
            additional_data: self.additional_data.clone(),
        }
    }

    /// Inserts a key-value pair into the additional data map.
    pub fn insert_data(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.additional_data.insert(key.into(), value.into());
        self
    }
}

impl FromStr for Header {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(';')
            .try_fold(Header::default(), |mut header, line| {
                if let Some(trace_id) = line.strip_prefix("Root=") {
                    header.trace_id = TraceId::Rendered(trace_id.into())
                } else if let Some(parent_id) = line.strip_prefix("Parent=") {
                    header.parent_id = Some(SegmentId::Rendered(parent_id.into()))
                } else if line.starts_with("Sampled=") {
                    header.sampling_decision = line.into();
                } else if !line.starts_with("Self=") {
                    let (key, value) = line
                        .split_once('=')
                        .ok_or_else(|| format!("invalid key=value: no `=` found in `{}`", s))?;
                    header.additional_data.insert(key.into(), value.into());
                }
                Ok(header)
            })
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Root={}", self.trace_id)?;
        if let Some(parent) = &self.parent_id {
            write!(f, ";Parent={}", parent)?;
        }
        if self.sampling_decision != SamplingDecision::Unknown {
            write!(f, ";{}", self.sampling_decision)?;
        }
        for (k, v) in &self.additional_data {
            write!(f, ";{}={}", k, v)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_with_parent_from_str() {
        assert_eq!(
            "Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1"
                .parse::<Header>(),
            Ok(Header {
                trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
                parent_id: Some(SegmentId::Rendered("53995c3f42cd8ad8".into())),
                sampling_decision: SamplingDecision::Sampled,
                ..Header::default()
            })
        )
    }
    #[test]
    fn parse_no_parent_from_str() {
        assert_eq!(
            "Root=1-5759e988-bd862e3fe1be46a994272793;Sampled=1".parse::<Header>(),
            Ok(Header {
                trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
                parent_id: None,
                sampling_decision: SamplingDecision::Sampled,
                ..Header::default()
            })
        )
    }
    #[test]
    fn parse_with_additional_data_from_str() {
        assert_eq!(
            "Root=1-5759e988-bd862e3fe1be46a994272793;Parent=53995c3f42cd8ad8;Sampled=1;Lineage=01234567:0;Unknown=unknown"
                .parse::<Header>(),
            Ok(Header {
                trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
                parent_id: Some(SegmentId::Rendered("53995c3f42cd8ad8".into())),
                sampling_decision: SamplingDecision::Sampled,
                additional_data: vec![
                    ("Lineage".into(), "01234567:0".into()),
                    ("Unknown".into(), "unknown".into()),
                ].into_iter().collect()
            })
        )
    }

    #[test]
    fn displays_as_header() {
        let header = Header {
            trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
            ..Header::default()
        };
        assert_eq!(
            format!("{}", header),
            "Root=1-5759e988-bd862e3fe1be46a994272793"
        );
    }

    #[test]
    fn replace_parent_id() {
        let header = Header {
            trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
            parent_id: Some(SegmentId::Rendered("53995c3f42cd8ad8".into())),
            sampling_decision: SamplingDecision::Sampled,
            ..Header::default()
        };
        assert_eq!(
            header.with_parent_id(SegmentId::Rendered("35b167406b7746cf".into())),
            Header {
                trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
                parent_id: Some(SegmentId::Rendered("35b167406b7746cf".into())),
                sampling_decision: SamplingDecision::Sampled,
                ..Header::default()
            },
        );
    }

    #[test]
    fn replace_sampling_decision() {
        let header = Header {
            trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
            parent_id: Some(SegmentId::Rendered("53995c3f42cd8ad8".into())),
            sampling_decision: SamplingDecision::Sampled,
            ..Header::default()
        };
        assert_eq!(
            header.with_sampling_decision(SamplingDecision::NotSampled),
            Header {
                trace_id: TraceId::Rendered("1-5759e988-bd862e3fe1be46a994272793".into()),
                parent_id: Some(SegmentId::Rendered("53995c3f42cd8ad8".into())),
                sampling_decision: SamplingDecision::NotSampled,
                ..Header::default()
            },
        );
    }
}
