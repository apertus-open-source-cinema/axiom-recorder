use itertools::Itertools;
use std::{collections::HashMap, fmt::Display};

pub fn code_with_line_numbers(code: &str) -> String {
    code.split("\n")
        .enumerate()
        .map(|(i, line)| {
            format!(
                "{} {}| {}",
                i + 1,
                " ".repeat(
                    code.matches("\n").count().to_string().len() - (i + 1).to_string().len()
                ),
                line
            )
        })
        .join("\n")
}


pub fn format_hash_map_option<K, V>(input: &HashMap<K, Option<V>>) -> String
where
    K: Display,
    V: Display,
{
    input
        .iter()
        .map(|(k, v)| match v {
            Some(v) => format!("{}: {}", k, v),
            None => format!("{}", k),
        })
        .join(", ")
}
