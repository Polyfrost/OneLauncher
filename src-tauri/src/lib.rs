use std::{env, f32::consts::E, path::{Path, PathBuf}, sync::Arc};

use rspc::Router;
use tokio::sync::broadcast;

pub mod auth;
pub mod game;
pub mod utils;
pub mod constants;

pub(crate) mod api;

// represents all data and stuff idk
// holds references to services like http and microsoft and stuff for ease of use
pub struct Node {
    pub config: something,
    pub data_dir: PathBuf,
    pub env: Arc<>,
}

impl Node {
    pub async fn new(
        data_dir: impl AsRef<Path>,
        env: idikdjasdgjsbana,
    ) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
        let data_dir = data_dir.as_ref();

        print!("starting with data dir '{}'", data_dir.display());

        let env = Arc::new(env);

        let _ = fs::create_dir_all(&data_dir).await;
        let event_bus = broadcast::channel(1024);
        

        let node = Arc::new(Node {
            data_dir: data_dir.to_path_buf(),
            env,
        });

        let router = api::mount();

        Ok((node, router));
    }
}

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("the error error error: {0}")]
    ErrorErrorError(String),
}
