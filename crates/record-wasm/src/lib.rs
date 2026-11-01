//! Browser bindings for the same pure domain engine used by the API and CLI.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn evaluate_report(state_json: &str) -> Result<String, JsValue> {
    let state: record_core::State =
        serde_json::from_str(state_json).map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&record_core::evaluate_report(&state))
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn counterfactual_approve(state_json: &str) -> Result<String, JsValue> {
    let state: record_core::State =
        serde_json::from_str(state_json).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let report = record_core::counterfactual_approve(&state)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&report).map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub fn compare_other_path(state_json: &str) -> Result<String, JsValue> {
    let state: record_core::State =
        serde_json::from_str(state_json).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let fixture: record_core::Fixture =
        serde_json::from_str(include_str!("../../../fixtures/role.json"))
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let comparison =
        record_core::compare_other_path(&record_core::state_from_fixture(&fixture), &state)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&comparison).map_err(|error| JsValue::from_str(&error.to_string()))
}
