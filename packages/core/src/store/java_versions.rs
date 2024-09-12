//! Handles all available Java installations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::utils::java;
use crate::utils::java::JavaVersion;

/// A `HashMap` of all located and installed available Java versions
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaVersions(HashMap<String, JavaVersion>);

impl Default for JavaVersions {
	fn default() -> Self {
		Self::new()
	}
}

impl JavaVersions {
	/// Create an empty `JavaVersions` `HashMap`.
	#[must_use]
	pub fn new() -> Self {
		Self(HashMap::new())
	}

	/// Inserts a key-value pair into the map.
	pub fn insert(&mut self, key: String, java: JavaVersion) {
		self.0.insert(key, java);
	}

	/// Removes a key from the map, returning the value at the key if the key was previously in the map.
	pub fn remove(&mut self, key: &String) {
		self.0.remove(key);
	}

	/// Returns a reference to the value corresponding to the key.
	#[must_use]
	pub fn get(&self, key: &String) -> Option<&JavaVersion> {
		self.0.get(key)
	}

	/// Returns a mutable reference to the value corresponding to the key.
	pub fn get_mut(&mut self, key: &String) -> Option<&mut JavaVersion> {
		self.0.get_mut(key)
	}

	/// Returns the number of elements in the map.
	#[must_use]
	pub fn count(&self) -> usize {
		self.0.len()
	}

	/// A collection visiting all keys in arbitrary order.
	#[must_use]
	pub fn keys(&self) -> Vec<String> {
		self.0.keys().cloned().collect()
	}

	/// Validates all stored java versions.
	pub async fn validate(&self) -> bool {
		for java in self.0.values() {
			let runtime = java::check_java_instance(PathBuf::from(&java.path).as_path()).await;
			if let Some(runtime) = runtime {
				if runtime.version != java.version {
					return false;
				}
			} else {
				return false;
			}
		}

		true
	}
}
