use hdk::prelude::*;

#[derive(Serialize, Deserialize, Clone, SerializedBytes, Debug)]
pub struct ExpressionProof {
    pub signature: String,
    pub key: String,
}

#[derive(Serialize, Deserialize, Clone, SerializedBytes, Debug)]
pub struct Triple {
    pub source: Option<String>,
    pub target: Option<String>,
    pub predicate: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, SerializedBytes, Debug)]
pub struct UriTag(pub String);
