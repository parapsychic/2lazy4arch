use std::collections::{HashMap, BTreeMap};

/* Contains utility functions and structs */

/* An optional partition table that can contain unique keys with unique values */
// I did not write this code. ChatGPT did. 
// Here is an explanation on how it works.
/// The `PartitionTable` struct contains two underlying maps:
/// - `key_to_value`: A `HashMap` for fast key-to-value lookups.
/// - `value_to_key`: A `BTreeMap` for ordered value-to-key lookups.
/// ### Insertion
/// When inserting a key-value pair, `PartitionTable` checks for uniqueness in both key and value. 
/// If the key or value already exists, the insertion fails, preventing duplicates.
/// ### Retrieval
/// You can retrieve values by providing keys or keys by providing values.
#[derive(Debug)]
pub struct PartitionTable {
    key_to_value: HashMap<String, String>,
    value_to_key: BTreeMap<String, String>, 
}

impl PartitionTable {
    pub fn new() -> Self {
        PartitionTable {
            key_to_value: HashMap::new(),
            value_to_key: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: String) -> Result<Option<String>, String> {
        if self.key_to_value.contains_key(&key) || self.value_to_key.contains_key(&value) {
            Err(format!("Key or value already exists"))
        } else {
            let old_value = self.key_to_value.insert(key.clone(), value.clone());
            self.value_to_key.insert(value, key);
            // return old value and replace it with new value.
            Ok(old_value)
        }
    }

    pub fn get_value(&self, key: &str) -> Option<&String> {
        self.key_to_value.get(key)
    }

    pub fn get_key(&self, value: &str) -> Option<&String> {
        self.value_to_key.get(value)
    }
}
