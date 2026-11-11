use morphrpc_compiler::{MAX_CONTRACT_BYTES, generate, parse};
use serde_json::{Value, json};

fn contract() -> Value {
    json!({ "schema_version": 1, "service": "Users", "messages": [
        {"name": "Request", "fields": [{"name": "id", "type": "u64"}]},
        {"name": "Response", "fields": [{"name": "name", "type": "string"}]}
    ], "methods": [{"name":"get_user", "operation_id":1, "request":"Request", "response":"Response"}] })
}
fn accepts(value: Value) -> bool {
    parse(&value.to_string()).is_ok()
}
#[test]
fn deterministic_output_and_strict_validation() {
    let input = contract().to_string();
    assert_eq!(
        generate(&parse(&input).unwrap()),
        generate(&parse(&input).unwrap())
    );
    let mut x = contract();
    x["schema_version"] = json!(2);
    assert!(!accepts(x));
    let mut x = contract();
    x["methods"][0]["operation_id"] = json!(0);
    assert!(!accepts(x));
    let mut x = contract();
    x["methods"][0]["operation_id"] = json!(65536);
    assert!(!accepts(x));
    let mut x = contract();
    x["messages"][0]["fields"][0]["type"] = json!("f32");
    assert!(!accepts(x));
    let mut x = contract();
    x["methods"][0]["response"] = json!("Missing");
    assert!(!accepts(x));
    let mut x = contract();
    x["messages"][0]["fields"][0]["name"] = json!("unsafe");
    assert!(!accepts(x));
    let mut x = contract();
    x["messages"][0]["name"] = json!("Reader");
    assert!(!accepts(x));
    let mut x = contract();
    x["methods"][0]["name"] = json!("new");
    assert!(!accepts(x));
    let mut x = contract();
    x["unknown"] = json!(true);
    assert!(!accepts(x));
    let mut x = contract();
    let duplicate = x["methods"][0].clone();
    x["methods"].as_array_mut().unwrap().push(duplicate);
    assert!(!accepts(x));
    let mut x = contract();
    let mut second = x["methods"][0].clone();
    second["name"] = json!("get_other");
    x["methods"].as_array_mut().unwrap().push(second);
    assert!(!accepts(x));
    let mut x = contract();
    let duplicate = x["messages"][0].clone();
    x["messages"].as_array_mut().unwrap().push(duplicate);
    assert!(!accepts(x));
    let mut x = contract();
    let duplicate = x["messages"][0]["fields"][0].clone();
    x["messages"][0]["fields"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    assert!(!accepts(x));
    let mut x = contract();
    x["messages"][0]["fields"][0]["name"] = json!("gen");
    assert!(!accepts(x));
    let mut x = contract();
    x["messages"][0]["fields"][0]["name"] = json!("twoWords");
    assert!(!accepts(x));
    assert!(parse(&" ".repeat(MAX_CONTRACT_BYTES + 1)).is_err());
}
