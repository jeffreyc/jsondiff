use clap::Parser;
use std::collections::HashSet;
use std::fmt;
use std::fs;

#[derive(Debug, PartialEq)]
enum JsonPatchOp {
    Add,
    Remove,
    Replace,
    // Move,
    // Copy,
    // Test,
}

impl fmt::Display for JsonPatchOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JsonPatchOp::Add => write!(f, "add"),
            JsonPatchOp::Remove => write!(f, "remove"),
            JsonPatchOp::Replace => write!(f, "replace"),
            // JsonPatchOp::Move => write!(f, "move"),
            // JsonPatchOp::Copy => write!(f, "copy"),
            // JsonPatchOp::Test => write!(f, "test"),
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    file1: String,
    file2: String,
}

#[derive(Debug, PartialEq)]
struct Patch {
    op: JsonPatchOp,
    path: String,
    value: Option<serde_json::Value>,
    old_value: Option<serde_json::Value>,
}

fn compare(left: &serde_json::Value, right: &serde_json::Value, patches: &mut Vec<Patch>) {
    if left.is_object() && right.is_object() {
        compare_objects(
            left.as_object().unwrap(),
            right.as_object().unwrap(),
            patches,
            None,
        );
    } else if left.is_array() && right.is_array() {
        compare_arrays(
            left.as_array().unwrap(),
            right.as_array().unwrap(),
            patches,
            None,
        );
    } else if left != right {
        patches.push(Patch {
            op: JsonPatchOp::Replace,
            path: "/".to_string(),
            value: Some(right.clone()),
            old_value: Some(left.clone()),
        });
    }
}

// Clippy flags `for i in left.len()..right.len()` but misses the non-zero start. Allow for now.
#[allow(clippy::needless_range_loop)]
fn compare_arrays(
    left: &Vec<serde_json::Value>,
    right: &Vec<serde_json::Value>,
    patches: &mut Vec<Patch>,
    prefix: Option<&str>,
) {
    if left != right {
        for i in 0..left.len() {
            if i < right.len() {
                if left[i] != right[i] {
                    if left[i].is_array() && right[i].is_array() {
                        compare_arrays(
                            left[i].as_array().unwrap(),
                            right[i].as_array().unwrap(),
                            patches,
                            Some(&*format!("{}/{}", prefix.unwrap_or(""), i)),
                        );
                    } else if left[i].is_object() && right[i].is_object() {
                        compare_objects(
                            left[i].as_object().unwrap(),
                            right[i].as_object().unwrap(),
                            patches,
                            Some(&*format!("{}/{}", prefix.unwrap_or(""), i)),
                        );
                    } else {
                        patches.push(Patch {
                            op: JsonPatchOp::Replace,
                            path: format!("{}/{}", prefix.unwrap_or(""), i),
                            value: Some(right[i].clone()),
                            old_value: Some(left[i].clone()),
                        });
                    }
                }
            } else {
                patches.push(Patch {
                    op: JsonPatchOp::Remove,
                    path: format!("{}/{}", prefix.unwrap_or(""), i),
                    value: None,
                    old_value: Some(left[i].clone()),
                });
            }
        }
        if right.len() > left.len() {
            for i in left.len()..right.len() {
                patches.push(Patch {
                    op: JsonPatchOp::Add,
                    path: format!("{}/{}", prefix.unwrap_or(""), i),
                    value: Some(right[i].clone()),
                    old_value: None,
                });
            }
        }
    }
}

fn compare_objects(
    left: &serde_json::Map<String, serde_json::Value>,
    right: &serde_json::Map<String, serde_json::Value>,
    patches: &mut Vec<Patch>,
    prefix: Option<&str>,
) {
    let left_keys: HashSet<String> = HashSet::from_iter(left.keys().cloned());
    let right_keys: HashSet<String> = HashSet::from_iter(right.keys().cloned());
    for key in left_keys.difference(&right_keys) {
        patches.push(Patch {
            op: JsonPatchOp::Remove,
            path: format!("{}/{}", prefix.unwrap_or(""), key),
            value: None,
            old_value: Some(left[key].clone()),
        });
    }
    for key in right_keys.difference(&left_keys) {
        patches.push(Patch {
            op: JsonPatchOp::Add,
            path: format!("{}/{}", prefix.unwrap_or(""), key),
            value: Some(right[key].clone()),
            old_value: None,
        });
    }
    for key in left_keys.intersection(&right_keys) {
        let old = left[key].clone();
        let new = right[key].clone();
        if old != new {
            if old.is_array() && new.is_array() {
                compare_arrays(
                    old.as_array().unwrap(),
                    new.as_array().unwrap(),
                    patches,
                    Some(&*format!("{}/{}", prefix.unwrap_or(""), key)),
                )
            } else if old.is_object() && new.is_object() {
                compare_objects(
                    old.as_object().unwrap(),
                    new.as_object().unwrap(),
                    patches,
                    Some(&*format!("{}/{}", prefix.unwrap_or(""), key)),
                );
            } else {
                patches.push(Patch {
                    op: JsonPatchOp::Replace,
                    path: format!("{}/{}", prefix.unwrap_or(""), key),
                    value: Some(new.clone()),
                    old_value: Some(old.clone()),
                });
            }
        }
    }
}

fn generate_json_patch(patches: &Vec<Patch>) -> Vec<String> {
    let mut ret: Vec<String> = Vec::new();
    for patch in patches {
        let json = if patch.op == JsonPatchOp::Remove {
            serde_json::json!({"op": "remove", "path": patch.path})
        } else {
            serde_json::json!({"op": patch.op.to_string(), "path": patch.path, "value": patch.value})
        };
        ret.push(json.to_string());
    }
    ret
}

fn get_and_parse_contents(file: String) -> serde_json::Value {
    let result = fs::read_to_string(&file);
    let contents = match result {
        Ok(contents) => contents,
        Err(error) => panic!("Could not open {}: {:?}", file, error),
    };
    match serde_json::from_str(contents.as_str()) {
        Ok(value) => value,
        Err(error) => panic!("Could not deserialize {}: {:?}", file, error),
    }
}

fn main() {
    let args = Args::parse();
    println!("Comparing {} and {}", args.file1, args.file2);

    let left = get_and_parse_contents(args.file1);
    let right = get_and_parse_contents(args.file2);

    let mut patches: Vec<Patch> = Vec::new();

    compare(&left, &right, &mut patches);
    if patches.is_empty() {
        println!("No differences were detected.");
    } else {
        println!("[");
        for (i, patch) in generate_json_patch(&patches).iter().enumerate() {
            let suffix = if i + 1 < patches.len() { "," } else { "" };
            println!("  {}{}", patch, suffix);
        }
        println!("]");
    }
}

#[cfg(test)]
mod tests {
    use crate::{compare, JsonPatchOp, Patch};

    #[test]
    fn test_compare_array_nop() {
        let doc = serde_json::json!(["a", "b"]);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&doc, &doc.clone(), &mut patches);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_compare_array_added_values() {
        let left = serde_json::json!(["a", "b"]);
        let right = serde_json::json!(["a", "b", "c", "d"]);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![
            Patch {
                op: JsonPatchOp::Add,
                path: "/2".to_string(),
                value: Some(serde_json::json!("c")),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/3".to_string(),
                value: Some(serde_json::json!("d")),
                old_value: None,
            },
        ];
        assert_eq!(expected, patches);
    }

    #[test]
    fn test_compare_array_changed_value() {
        let left = serde_json::json!(["a", "b", "c"]);
        let right = serde_json::json!(["a", 2, "c"]);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![Patch {
            op: JsonPatchOp::Replace,
            path: "/1".to_string(),
            value: Some(serde_json::json!(2)),
            old_value: Some(serde_json::json!("b")),
        }];
        assert_eq!(expected, patches);
    }

    #[test]
    fn test_compare_array_changed_nested_value() {
        let left = serde_json::json!(["a", ["b", "c"], "d"]);
        let right = serde_json::json!(["a", ["b", 3], "d"]);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![Patch {
            op: JsonPatchOp::Replace,
            path: "/1/1".to_string(),
            value: Some(serde_json::json!(3)),
            old_value: Some(serde_json::json!("c")),
        }];
        assert_eq!(expected, patches);
    }

    #[test]
    fn test_compare_array_removed_value() {
        let left = serde_json::json!(["a", "b", "c", "d"]);
        let right = serde_json::json!(["a", "b"]);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![
            Patch {
                op: JsonPatchOp::Remove,
                path: "/2".to_string(),
                value: None,
                old_value: Some(serde_json::json!("c")),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/3".to_string(),
                value: None,
                old_value: Some(serde_json::json!("d")),
            },
        ];
        assert_eq!(expected, patches);
    }

    #[test]
    fn test_compare_object_nop() {
        let doc = serde_json::json!({
            "string": "This is a string.",
            "integer": 42,
            "float": 3.14159,
            "object": {
                "substring": "This is another string."
            },
            "array": ["one", "two"],
            "boolean": true,
            "null": null
        });
        let mut patches: Vec<Patch> = Vec::new();
        compare(&doc, &doc.clone(), &mut patches);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_compare_object_added() {
        let left = serde_json::json!({});
        let right = serde_json::json!({
            "string": "This is a string.",
            "integer": 42,
            "float": 3.14159,
            "object": {
                "substring": "This is another string."
            },
            "array": ["one", "two"],
            "boolean": true,
            "null": null
        });
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right, &mut patches);
        let expected = vec![
            Patch {
                op: JsonPatchOp::Add,
                path: "/string".to_string(),
                value: Some(serde_json::json!("This is a string.")),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/integer".to_string(),
                value: Some(serde_json::json!(42)),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/float".to_string(),
                value: Some(serde_json::json!(3.14159)),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/object".to_string(),
                value: Some(serde_json::json!({"substring": "This is another string."})),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/array".to_string(),
                value: Some(serde_json::json!(["one", "two"])),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/boolean".to_string(),
                value: Some(serde_json::json!(true)),
                old_value: None,
            },
            Patch {
                op: JsonPatchOp::Add,
                path: "/null".to_string(),
                value: Some(serde_json::json!(null)),
                old_value: None,
            },
        ];
        assert_eq!(expected.len(), patches.len());
        assert!(expected.iter().all(|item| patches.contains(item)));
    }

    #[test]
    fn test_compare_object_removed() {
        let left = serde_json::json!({
            "string": "This is a string.",
            "integer": 42,
            "float": 3.14159,
            "object": {
                "substring": "This is another string."
            },
            "array": ["one", "two"],
            "boolean": true,
            "null": null
        });
        let right = serde_json::json!({});
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right, &mut patches);
        let expected = vec![
            Patch {
                op: JsonPatchOp::Remove,
                path: "/string".to_string(),
                value: None,
                old_value: Some(serde_json::json!("This is a string.")),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/integer".to_string(),
                value: None,
                old_value: Some(serde_json::json!(42)),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/float".to_string(),
                value: None,
                old_value: Some(serde_json::json!(3.14159)),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/object".to_string(),
                value: None,
                old_value: Some(serde_json::json!({"substring": "This is another string."})),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/array".to_string(),
                value: None,
                old_value: Some(serde_json::json!(["one", "two"])),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/boolean".to_string(),
                value: None,
                old_value: Some(serde_json::json!(true)),
            },
            Patch {
                op: JsonPatchOp::Remove,
                path: "/null".to_string(),
                value: None,
                old_value: Some(serde_json::json!(null)),
            },
        ];
        assert_eq!(expected.len(), patches.len());
        assert!(expected.iter().all(|item| patches.contains(item)));
    }

    #[test]
    fn test_compare_object_replaced() {
        let left = serde_json::json!({
            "string": "There are strange things done in the midnight sun",
            "integer": 42,
            "float": 3.14159,
            "object": {
                "substring": "By the men who moil for gold;"
            },
            "array": ["one", "two"],
            "boolean": true,
            "null": null
        });
        let right = serde_json::json!({
            "string": "The Arctic trails have their secret tales",
            "integer": 60606,
            "float": 2.71828,
            "object": {
                "substring": "That would make your blood run cold;"
            },
            "array": ["a", "b"],
            "boolean": false,
            "null": "NOT NULL"
        });
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right, &mut patches);
        let expected = vec![
            Patch {
                op: JsonPatchOp::Replace,
                path: "/string".to_string(),
                value: Some(serde_json::json!(
                    "The Arctic trails have their secret tales"
                )),
                old_value: Some(serde_json::json!(
                    "There are strange things done in the midnight sun"
                )),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/integer".to_string(),
                value: Some(serde_json::json!(60606)),
                old_value: Some(serde_json::json!(42)),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/float".to_string(),
                value: Some(serde_json::json!(2.71828)),
                old_value: Some(serde_json::json!(3.14159)),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/object/substring".to_string(),
                value: Some(serde_json::json!("That would make your blood run cold;")),
                old_value: Some(serde_json::json!("By the men who moil for gold;")),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/array/0".to_string(),
                value: Some(serde_json::json!("a")),
                old_value: Some(serde_json::json!("one")),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/array/1".to_string(),
                value: Some(serde_json::json!("b")),
                old_value: Some(serde_json::json!("two")),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/boolean".to_string(),
                value: Some(serde_json::json!(false)),
                old_value: Some(serde_json::json!(true)),
            },
            Patch {
                op: JsonPatchOp::Replace,
                path: "/null".to_string(),
                value: Some(serde_json::json!("NOT NULL")),
                old_value: Some(serde_json::json!(null)),
            },
        ];
        assert_eq!(expected.len(), patches.len());
        assert!(expected.iter().all(|item| patches.contains(item)));
    }

    #[test]
    fn test_compare_string_nop() {
        let doc = serde_json::json!("There are strange things done in the midnight sun");
        let mut patches: Vec<Patch> = Vec::new();
        compare(&doc, &doc.clone(), &mut patches);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_compare_string_changed_value() {
        let left = serde_json::json!("There are strange things done in the midnight sun");
        let right = serde_json::json!("By the men who moil for gold;");
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![Patch {
            op: JsonPatchOp::Replace,
            path: "/".to_string(),
            value: Some(serde_json::json!("By the men who moil for gold;")),
            old_value: Some(serde_json::json!(
                "There are strange things done in the midnight sun"
            )),
        }];
        assert_eq!(expected, patches);
    }

    #[test]
    fn test_compare_string_changed_type() {
        let left = serde_json::json!("There are strange things done in the midnight sun");
        let right = serde_json::json!(42);
        let mut patches: Vec<Patch> = Vec::new();
        compare(&left, &right.clone(), &mut patches);
        let expected = vec![Patch {
            op: JsonPatchOp::Replace,
            path: "/".to_string(),
            value: Some(serde_json::json!(42)),
            old_value: Some(serde_json::json!(
                "There are strange things done in the midnight sun"
            )),
        }];
        assert_eq!(expected, patches);
    }
}
