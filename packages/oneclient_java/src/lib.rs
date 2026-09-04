//! [`JavaService`] takes a [`JavaStore`] rather than a database handle so this
//! crate has no schema dependency

mod checker;
mod data;
mod error;
mod install;
mod locate;
mod platform;
mod resolve;
mod service;
mod store;

pub mod vendors;

pub use checker::{JavaCheckInfo, check_java_runtime};
pub use data::{JavaPackage, JavaRuntime, PackageArchive, java_executable_relative_path};
pub use error::{JavaError, JavaResult};
pub use install::install_package;
pub use locate::{LocatedJava, best_for_major, locate_java};
pub use platform::{HostArch, HostOs, HostTarget};
pub use service::{
	AvailableJava, INSTALLABLE_MAJORS, JAVA_CHOICE_DOWNLOAD, JAVA_CHOICE_FOLDER, JAVA_VENDOR_HINT,
	JavaService,
};
pub use store::{JavaStore, MemoryJavaStore, StoreError, StoreResult};
pub use vendors::JavaVendor;
