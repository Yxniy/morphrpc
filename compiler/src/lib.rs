//! Validation and deterministic Rust generation for MorphRPC service contracts.
#![forbid(unsafe_code)]

use serde::Deserialize;
use std::{collections::HashSet, fmt::Write};

/// Maximum UTF-8 contract size accepted by the compiler.
pub const MAX_CONTRACT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    schema_version: u32,
    service: String,
    messages: Vec<Message>,
    methods: Vec<Method>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    name: String,
    fields: Vec<Field>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    kind: FieldType,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FieldType {
    U64,
    U32,
    Bool,
    String,
    Bytes,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Method {
    name: String,
    operation_id: u16,
    request: String,
    response: String,
}

/// An immutable contract that passed all compiler checks.
pub struct ValidatedContract(Contract);

fn identifier(value: &str, type_name: bool) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    let starts_correctly = if type_name {
        first.is_ascii_uppercase()
    } else {
        first.is_ascii_lowercase()
    };
    let valid = starts_correctly
        && bytes.all(|b| {
            if type_name {
                b.is_ascii_alphanumeric()
            } else {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
            }
        });
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true",
        "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final",
        "gen", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
    ];
    valid && !KEYWORDS.contains(&value)
}
fn validate(contract: Contract) -> Result<ValidatedContract, String> {
    if contract.schema_version != 1 {
        return Err("unsupported schema version".into());
    }
    if !identifier(&contract.service, true) {
        return Err("invalid service name".into());
    }
    if contract.messages.is_empty() || contract.methods.is_empty() {
        return Err("at least one message and method are required".into());
    }
    let reserved = [
        format!("{}Client", contract.service),
        format!("{}Service", contract.service),
        "RpcError".to_owned(),
        "PayloadError".to_owned(),
        "Reader".to_owned(),
        "String".to_owned(),
        "Vec".to_owned(),
        "Result".to_owned(),
        "Option".to_owned(),
        "Some".to_owned(),
        "Ok".to_owned(),
        "Err".to_owned(),
        "Self".to_owned(),
    ];
    let mut names = HashSet::new();
    for message in &contract.messages {
        if !identifier(&message.name, true)
            || reserved.contains(&message.name)
            || !names.insert(message.name.as_str())
        {
            return Err(format!(
                "invalid or duplicate message name: {}",
                message.name
            ));
        }
        let mut fields = HashSet::new();
        for field in &message.fields {
            if !identifier(&field.name, false) || !fields.insert(field.name.as_str()) {
                return Err(format!("invalid or duplicate field: {}", field.name));
            }
        }
    }
    let mut methods = HashSet::new();
    let mut ids = HashSet::new();
    for method in &contract.methods {
        if !identifier(&method.name, false)
            || method.name == "new"
            || !methods.insert(method.name.as_str())
        {
            return Err(format!("invalid or duplicate method: {}", method.name));
        }
        if method.operation_id == 0 || !ids.insert(method.operation_id) {
            return Err("operation IDs must be unique and nonzero".into());
        }
        if !names.contains(method.request.as_str()) || !names.contains(method.response.as_str()) {
            return Err(format!(
                "unknown message reference in method {}",
                method.name
            ));
        }
    }
    Ok(ValidatedContract(contract))
}

/// Parse and validate a versioned JSON contract.
pub fn parse(input: &str) -> Result<ValidatedContract, String> {
    if input.len() > MAX_CONTRACT_BYTES {
        return Err("contract exceeds maximum size".into());
    }
    let contract =
        serde_json::from_str(input).map_err(|e| format!("invalid contract JSON: {e}"))?;
    validate(contract)
}

/// Generate a self-contained Rust module using MorphRPC's v1 runtime.
pub fn generate(contract: &ValidatedContract) -> String {
    let contract = &contract.0;
    let mut out = String::from(include_str!("generated_support.rs.txt"));
    for message in &contract.messages {
        writeln!(
            out,
            "\n#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {} {{",
            message.name
        )
        .unwrap();
        for field in &message.fields {
            let ty = match field.kind {
                FieldType::U64 => "u64",
                FieldType::U32 => "u32",
                FieldType::Bool => "bool",
                FieldType::String => "String",
                FieldType::Bytes => "Vec<u8>",
            };
            writeln!(out, "    pub {}: {},", field.name, ty).unwrap();
        }
        let mutable = if message.fields.is_empty() {
            ""
        } else {
            "mut "
        };
        writeln!(out, "}}\nimpl {} {{\n    pub fn encode(&self) -> Result<Vec<u8>, PayloadError> {{\n        let {mutable}output = Vec::new();", message.name).unwrap();
        for field in &message.fields {
            match field.kind {
                FieldType::U64 | FieldType::U32 => writeln!(
                    out,
                    "        append(&mut output, &self.{}.to_be_bytes())?;",
                    field.name
                )
                .unwrap(),
                FieldType::Bool => writeln!(
                    out,
                    "        append(&mut output, &[u8::from(self.{})])?;",
                    field.name
                )
                .unwrap(),
                FieldType::String => writeln!(
                    out,
                    "        append_blob(&mut output, self.{}.as_bytes())?;",
                    field.name
                )
                .unwrap(),
                FieldType::Bytes => writeln!(
                    out,
                    "        append_blob(&mut output, &self.{})?;",
                    field.name
                )
                .unwrap(),
            }
        }
        writeln!(out, "        Ok(output)\n    }}\n    pub fn decode(input: &[u8]) -> Result<Self, PayloadError> {{\n        if input.len() > MAX_PAYLOAD {{ return Err(PayloadError::TooLarge); }}\n        let {mutable}reader = Reader {{ input, offset: 0 }};\n        let value = Self {{").unwrap();
        for field in &message.fields {
            let expr = match field.kind {
                FieldType::U64 => "u64::from_be_bytes(reader.take(8)?.try_into().unwrap())",
                FieldType::U32 => "u32::from_be_bytes(reader.take(4)?.try_into().unwrap())",
                FieldType::Bool => {
                    "match reader.take(1)?[0] { 0 => false, 1 => true, _ => return Err(PayloadError::InvalidBoolean) }"
                }
                FieldType::String => {
                    "std::str::from_utf8(reader.blob()?).map_err(|_| PayloadError::InvalidUtf8)?.to_owned()"
                }
                FieldType::Bytes => "reader.blob()?.to_vec()",
            };
            writeln!(out, "            {}: {},", field.name, expr).unwrap();
        }
        out.push_str("        };\n        if reader.offset != input.len() { return Err(PayloadError::TrailingBytes); }\n        Ok(value)\n    }\n}\n");
    }
    writeln!(out, "\npub struct {}Client {{ inner: morphrpc::runtime::Client }}\nimpl {}Client {{\n    pub fn new(inner: morphrpc::runtime::Client) -> Self {{ Self {{ inner }} }}", contract.service, contract.service).unwrap();
    for method in &contract.methods {
        writeln!(out, "    pub async fn {}(&mut self, request: &{}) -> Result<{}, RpcError> {{\n        let response = self.inner.call({}, request.encode().map_err(RpcError::Payload)?).await.map_err(RpcError::Transport)?;\n        match response.kind {{\n            morphrpc::protocol::MessageKind::Response => {}::decode(&response.payload).map_err(RpcError::Payload),\n            morphrpc::protocol::MessageKind::Error => Err(RpcError::Remote(response.payload)),\n            morphrpc::protocol::MessageKind::Request => Err(RpcError::UnexpectedResponse),\n        }}\n    }}", method.name, method.request, method.response, method.operation_id, method.response).unwrap();
    }
    writeln!(out, "}}\npub trait {}Service {{", contract.service).unwrap();
    for method in &contract.methods {
        writeln!(
            out,
            "    fn {}(&self, request: {}) -> Result<{}, String>;",
            method.name, method.request, method.response
        )
        .unwrap();
    }
    writeln!(out, "}}\npub fn dispatch<__MorphHandler: {}Service>(handler: &__MorphHandler, operation: u16, payload: &[u8]) -> (morphrpc::protocol::MessageKind, Vec<u8>) {{\n    let result: Result<Vec<u8>, &'static str> = match operation {{", contract.service).unwrap();
    for method in &contract.methods {
        writeln!(out, "        {} => {}::decode(payload).map_err(|_| \"invalid payload\").and_then(|request| handler.{}(request).map_err(|_| \"handler error\")).and_then(|response| response.encode().map_err(|_| \"response too large\")),", method.operation_id, method.request, method.name).unwrap();
    }
    out.push_str("        _ => Err(\"unknown operation\"),\n    };\n    match result {\n        Ok(bytes) => (morphrpc::protocol::MessageKind::Response, bytes),\n        Err(error) => (morphrpc::protocol::MessageKind::Error, error.as_bytes().to_vec()),\n    }\n}\n");
    out
}
