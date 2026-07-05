
use freya::prelude::*;

struct MinApp;

impl App for MinApp {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(label().text("hello"))
    }
}

fn main() {
    let cache_mb = std::env::var("ONECLIENT_GPU_CACHE_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(128);

    let config = LaunchConfig::new()
        .with_window(
            WindowConfig::new_app(MinApp)
                .with_title("min_freya")
                .with_size(1200., 800.),
        )
        .with_gpu_resource_cache_limit(cache_mb * 1024 * 1024);

    launch(config);
}
