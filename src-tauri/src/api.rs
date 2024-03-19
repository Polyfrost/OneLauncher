use std::sync::Arc;

use crate::Node;

pub type Ctx = Arc<Node>;
pub type Router = rspc::Router<Ctx>;

#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
    InvalidateOperation(),
    MSALogin(),
}

pub(crate) fn mount() -> Arc<Router> {
    let r = R
        .router();

    let r = r.build({
        let config = rspc::ExportConfig::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/client.ts")).set_header("/* eslint-disable */");
        config
    });

    r
}

#[cfg(test)]
mod tests {
    #[test]
    fn export() {
        super::mount();
    }
}
