use std::collections::BTreeSet;

use futures_util::StreamExt;

use crate::images::{DEFAULT_IMAGE_EDGE, PREVIEW_IMAGE_EDGE};
use crate::state::LauncherState;

const PREFETCH_CONCURRENCY: usize = 4;

const PREFETCH_EDGES: [u32; 2] = [PREVIEW_IMAGE_EDGE, DEFAULT_IMAGE_EDGE];

#[tracing::instrument(level = "debug", skip(state))]
pub async fn prefetch_version_art(state: &LauncherState) {
    let meta_url_base = state.services.requester.config().meta_url_base.clone();

    let mut urls: BTreeSet<String> = state
        .versions
        .arts(&meta_url_base)
        .await
        .gallery()
        .into_iter()
        .collect();

    urls.extend(
        state
            .versions
            .metadata(&meta_url_base)
            .await
            .into_iter()
            .filter_map(|entry| entry.art_url),
    );

    if urls.is_empty() {
        tracing::debug!("no version art to prefetch");
        return;
    }

    let total = urls.len();
    let mut failed = 0usize;

    let mut tasks = futures_util::stream::iter(urls)
        .map(|url| async move {
            let outcome = state
                .images
                .refresh(&state.services.requester, &url, &PREFETCH_EDGES)
                .await;
            (url, outcome)
        })
        .buffer_unordered(PREFETCH_CONCURRENCY);

    while let Some((url, outcome)) = tasks.next().await {
        if let Err(err) = outcome {
            failed += 1;
            tracing::debug!("version art prefetch failed for {url}: {err}");
        }
    }

    if failed > 0 {
        tracing::warn!("version art prefetch finished with {failed} of {total} unavailable");
    } else {
        tracing::debug!("version art prefetch warmed {total} images");
    }
}
