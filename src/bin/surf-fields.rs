//! `surf-fields` — blocks as named fields, over JSON.
//!
//! One JSON request on stdin, one JSON response on stdout (see
//! `surf_parse::fields::handle`). Exit 0 when the request was understood, even
//! when ops were refused; exit 2 with `{"error": "..."}` when it was not.
//! Convenience form: `surf-fields admit <file.surf>...` runs the admission
//! gate over files named as arguments.

use serde_json::{json, Value};
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request: Result<Value, String> = if args.first().map(String::as_str) == Some("admit") {
        args[1..]
            .iter()
            .map(|path| std::fs::read_to_string(path).map(|source| json!({"name": path, "source": source})).map_err(|e| format!("{path}: {e}")))
            .collect::<Result<Vec<_>, _>>()
            .map(|sources| json!({"cmd": "admit", "sources": sources}))
    } else {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| e.to_string())
            .and_then(|_| serde_json::from_str(&input).map_err(|e| format!("the request is not JSON: {e}")))
    };
    match request.and_then(|r| surf_parse::fields::handle(&r)) {
        Ok(response) => println!("{response}"),
        Err(error) => {
            println!("{}", json!({ "error": error }));
            std::process::exit(2);
        }
    }
}
