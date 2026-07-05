mod error;
mod checker;
mod data;
mod locate;
mod manager;
mod resolve;
mod install;
pub mod vendors;

pub(super) use error::JavaResult;
pub use error::JavaError;
pub use checker::*;
pub use data::{JavaPackage, JavaRuntime, PackageArchive};
pub use manager::{AvailableJava, JavaManager, INSTALLABLE_MAJORS};
pub use vendors::JavaVendor;
pub use install::install_package;