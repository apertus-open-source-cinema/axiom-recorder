use crate::util::error::{Error, Res};
use clap::ArgMatches;
use std::{any::Any, collections::HashMap, str::FromStr};

pub struct OptionsStorage(pub HashMap<String, String>);

impl OptionsStorage {
    pub fn from_args(arguments: ArgMatches, to_extract: Vec<&str>) -> Self {
        let mut options = HashMap::new();
        for prop in to_extract {
            if arguments.is_present(prop) {
                match arguments.value_of(prop.clone()) {
                    Some(value) => {
                        options.insert(String::from(prop), String::from(value));
                    }
                    None => {
                        options.insert(String::from(prop), String::from(""));
                    }
                }
            }
        }
        OptionsStorage(options)
    }

    pub fn get_opt(&self, key: &str) -> Res<String> {
        let got = &self.0.get(key);
        if got.is_none() {
            Error::error(format!("please specify {} as an option, since it is needed", key))?
        }
        Ok(String::from(got.unwrap()))
    }

    pub fn get_opt_or(&self, key: &str, alternative: &str) -> String {
        let got = &self.0.get(key);
        String::from(got.unwrap_or(&String::from(alternative)))
    }

    pub fn get_opt_parse<T>(&self, key: &str) -> Res<T>
    where
        T: 'static + FromStr,
        <T as std::str::FromStr>::Err: std::error::Error,
    {
        Ok(self.get_opt(key)?.parse::<T>()?)
    }

    pub fn is_present(&self, key: &str) -> bool { *&self.0.contains_key(key) }
}
