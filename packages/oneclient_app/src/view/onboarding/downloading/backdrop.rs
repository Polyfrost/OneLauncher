use super::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use oneclient_core::clusters::Cluster;

use crate::components::DynamicArt;
use crate::theme::colors;

const BACKDROP_INTERVAL_SECS: u64 = 12;

#[derive(PartialEq)]
pub struct LoadingBackdrop {
    pub clusters: Vec<Cluster>,
}

impl Component for LoadingBackdrop {
    fn render(&self) -> impl IntoElement {
        let clusters = &self.clusters;
        let count = clusters.len().max(1);
        let mut index = use_state(|| 0usize);

        let stop = use_hook(|| Arc::new(AtomicBool::new(false)));
        
        use_drop({
            let stop = stop.clone();
            move || stop.store(true, Ordering::Relaxed)
        });

        use_hook({
            let stop = stop.clone();
            move || {
                if count <= 1 {
                    return;
                }

                spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_secs(BACKDROP_INTERVAL_SECS)).await;

                        if stop.load(Ordering::Relaxed) {
                            break;
                        }

                        let next = (*index.peek() + 1) % count;
                        index.set(next);
                    }
                });
            }
        });

        let current = *index.read() % count;

        let fade = use_animation_with_dependencies(&current, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0.0, 1.0)
                .time(900)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });

        let art: Element = match clusters.get(current) {
            Some(cluster) => rect()
                .width(Size::fill())
                .height(Size::fill())
                .opacity(fade.get().value())
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::fill())
                        .child(DynamicArt::for_cluster(cluster).max_edge(1280)),
                )
                .into_element(),

            None => rect()
                .width(Size::fill())
                .height(Size::fill())
                .background(colors::page())
                .into_element(),
        };

        rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .interactive(false)
            .background(colors::page())
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute().top(0.).left(0.))
                    .child(art),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(8))
                    .background(
                        LinearGradient::new()
                            .angle(270.)
                            .stop((Color::BLACK.with_a(235), 0.))
                            .stop((Color::BLACK.with_a(235), 32.))
                            .stop((Color::BLACK.with_a(120), 48.))
                            .stop((Color::BLACK.with_a(10), 100.)),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute().top(0.).left(0.))
                    .layer(Layer::Relative(8))
                    .background(
                        LinearGradient::new()
                            .angle(0.)
                            .stop((Color::BLACK.with_a(0), 55.))
                            .stop((Color::BLACK.with_a(160), 100.)),
                    ),
            )
    }
}

