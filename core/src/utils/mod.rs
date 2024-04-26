//! OneLauncher utility functions
pub mod io;
pub mod http;
pub mod platform;
pub mod java;
pub mod watcher;
pub mod pkg;

/// mutable reference gets epically owned by free thinking macro!!!! (not clickbait)
/// im going insane insane insane insane insane insane
macro_rules! ref_owned {
    ($id:ident = $init:expr => $transform:block) => {{
        let mut it = $init;
        {
            let $id = &mut it;
            $transform;
        }
        it
    }};
}
