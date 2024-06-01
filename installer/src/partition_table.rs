use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap};

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
/// Here, key is the mount point and value is the device
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

 pub fn iter(&self) -> std::vec::IntoIter<(&String, &String)> {
        let mut entries: Vec<(&String, &String)> = self.key_to_value.iter().collect();
        
        // Sort the vector based on the length of the keys
        // This is done so that nested mounts like /home/mount will be 
        // executed after /home is mounted
        entries.sort_by(|a, b| a.0.len().cmp(&b.0.len()));

        entries.into_iter()
    }

    pub fn clear(&mut self) {
        self.value_to_key.clear();
        self.key_to_value.clear();
    }

    /// Removes the partition mount from the table
    /// Key gets precedence
    pub fn remove(&mut self, key: Option<&str>, value: Option<&str>) -> Result<()> {
        if let Some(x) = key {
             if let Ok(_) = self.remove_key(x) {
                return Ok(())
            }
        };

        if let Some(x) = value {
             if let Ok(_) = self.remove_value(x) {
                return Ok(())
            }
        };

        Err(anyhow!("Both key and value were either None or not found."))
    }

    pub fn remove_key(&mut self, key: &str) -> Result<String> {
        match self.key_to_value.remove(key) {
            Some(x) => {
                self.value_to_key.remove(&x);
                Ok(x)
            }
            None => Err(anyhow!("{} does not exist", key)),
        }
    }

    pub fn remove_value(&mut self, value: &str) -> Result<String> {
        match self.value_to_key.remove(value) {
            Some(x) => {
                self.key_to_value.remove(&x);
                Ok(x)
            }
            None => Err(anyhow!("{} does not exist", value)),
        }
    }

    pub fn insert(&mut self, key: String, value: String) -> Result<()> {
        let fmt_key = if key.starts_with('/') {
            &key[1..]
        } else {
            &key[..]
        };

        if self.key_to_value.contains_key(fmt_key) || self.value_to_key.contains_key(&value) {
            Err(anyhow!(
                "Mount point or partition already mounted. {}: {}",
                self.value_to_key.get(&value).unwrap_or(&String::new()),
                value
            ))
        } else {
            self.key_to_value
                .insert(fmt_key.to_string().clone(), value.clone());
            self.value_to_key.insert(value, fmt_key.to_string());
            Ok(())
        }
    }

    pub fn get_value(&self, key: &str) -> Option<&String> {
        self.key_to_value.get(key)
    }

    pub fn get_key(&self, value: &str) -> Option<&String> {
        self.value_to_key.get(value)
    }
}
