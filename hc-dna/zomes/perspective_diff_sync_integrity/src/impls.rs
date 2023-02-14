use hdk::prelude::*;

use crate::{
    Anchor, OnlineAgent, PerspectiveDiff, PerspectiveDiffReference, PerspectiveExpression,
};

impl PerspectiveDiff {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into()
            .map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}

impl PerspectiveDiffReference {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into()
            .map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}

impl Anchor {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into()
            .map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}

impl PerspectiveExpression {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into()
            .map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}

impl OnlineAgent {
    pub fn get_sb(self) -> ExternResult<SerializedBytes> {
        self.try_into()
            .map_err(|error| wasm_error!(WasmErrorInner::Host(String::from(error))))
    }
}
