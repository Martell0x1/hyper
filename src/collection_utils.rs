use indexmap::IndexMap;

use crate::environment::HyperValue;
use crate::error;

fn expect_args(method: &str, args: &[HyperValue], wanted: usize, line: u32) {
    if args.len() != wanted {
        error::runtime(
            line,
            format!(
                "{} expects {} argument(s) but got {}",
                method,
                wanted,
                args.len()
            ),
        );
    }
}

pub fn call_list_method(
    items: &mut Vec<HyperValue>,
    method: &str,
    args: &[HyperValue],
    line: u32,
) -> Option<HyperValue> {
    match method {
        "len" => {
            expect_args("len", args, 0, line);
            Some(HyperValue::I64(items.len() as i64))
        }
        "append" => {
            expect_args("append", args, 1, line);
            items.push(args[0].clone());
            Some(HyperValue::None)
        }
        other => error::runtime(line, format!("list has no method '{}'", other)),
    }
}

pub fn call_dict_method(
    entries: &mut IndexMap<String, HyperValue>,
    method: &str,
    args: &[HyperValue],
    line: u32,
) -> Option<HyperValue> {
    match method {
        "len" => {
            expect_args("len", args, 0, line);
            Some(HyperValue::I64(entries.len() as i64))
        }
        "keys" => {
            expect_args("keys", args, 0, line);
            let keys = entries
                .keys()
                .map(|k| HyperValue::String(k.clone()))
                .collect();
            Some(HyperValue::List(keys))
        }
        other => error::runtime(line, format!("dict has no method '{}'", other)),
    }
}
