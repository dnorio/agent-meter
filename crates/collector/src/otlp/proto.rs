// OTLP Trace protobuf types — generated from opentelemetry-proto spec
// Messages follow the ExportTraceServiceRequest schema.

#[derive(Clone, PartialEq, Message)]
pub struct ExportTraceServiceRequest {
    #[prost(message, repeated, tag = 1)]
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResourceSpans {
    #[prost(message, optional, tag = 1)]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = 2)]
    pub scope_spans: Vec<ScopeSpans>,
    #[prost(string, tag = 3)]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Resource {
    #[prost(message, repeated, tag = 1)]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = 2)]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ScopeSpans {
    #[prost(message, optional, tag = 1)]
    pub scope: Option<InstrumentationScope>,
    #[prost(message, repeated, tag = 2)]
    pub spans: Vec<Span>,
    #[prost(string, tag = 3)]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct InstrumentationScope {
    #[prost(string, tag = 1)]
    pub name: String,
    #[prost(string, tag = 2)]
    pub version: String,
    #[prost(message, repeated, tag = 3)]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = 4)]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct Span {
    #[prost(bytes, tag = 1)]
    pub trace_id: Vec<u8>,
    #[prost(bytes, tag = 2)]
    pub span_id: Vec<u8>,
    #[prost(string, tag = 3)]
    pub trace_state: String,
    #[prost(bytes, tag = 4)]
    pub parent_span_id: Vec<u8>,
    #[prost(string, tag = 5)]
    pub name: String,
    #[prost(int32, tag = 6)]
    pub kind: i32,
    #[prost(fixed64, tag = 7)]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = 8)]
    pub end_time_unix_nano: u64,
    #[prost(message, repeated, tag = 9)]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = 10)]
    pub dropped_attributes_count: u32,
    #[prost(message, repeated, tag = 11)]
    pub events: Vec<SpanEvent>,
    #[prost(message, repeated, tag = 12)]
    pub links: Vec<SpanLink>,
    #[prost(message, optional, tag = 13)]
    pub status: Option<Status>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SpanEvent {
    #[prost(fixed64, tag = 1)]
    pub time_unix_nano: u64,
    #[prost(string, tag = 2)]
    pub name: String,
    #[prost(message, repeated, tag = 3)]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = 4)]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct SpanLink {
    #[prost(bytes, tag = 1)]
    pub trace_id: Vec<u8>,
    #[prost(bytes, tag = 2)]
    pub span_id: Vec<u8>,
    #[prost(string, tag = 3)]
    pub trace_state: String,
    #[prost(message, repeated, tag = 4)]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = 5)]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct KeyValue {
    #[prost(string, tag = 1)]
    pub key: String,
    #[prost(message, optional, tag = 2)]
    pub value: Option<AnyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AnyValue {
    #[prost(oneof = "any_value::Value", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub value: Option<any_value::Value>,
}

pub mod any_value {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(string, tag = 1)]
        StringValue(String),
        #[prost(bool, tag = 2)]
        BoolValue(bool),
        #[prost(int64, tag = 3)]
        IntValue(i64),
        #[prost(double, tag = 4)]
        DoubleValue(f64),
        #[prost(message, tag = 5)]
        ArrayValue(super::ArrayValue),
        #[prost(message, tag = 6)]
        KvlistValue(super::KeyValueList),
        #[prost(bytes, tag = 7)]
        BytesValue(Vec<u8>),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ArrayValue {
    #[prost(message, repeated, tag = 1)]
    pub values: Vec<AnyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct KeyValueList {
    #[prost(message, repeated, tag = 1)]
    pub values: Vec<KeyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Status {
    #[prost(int32, tag = 1)]
    pub code: i32,
    #[prost(string, tag = 2)]
    pub message: String,
}
