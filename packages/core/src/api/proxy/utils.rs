use futures::prelude::*;

use crate::proxy::send::send_ingress;
use crate::proxy::IngressId;

#[macro_export]
macro_rules! count {
    () => (0usize);
    ( $x:tt $($xs:tt)* ) => (1usize + $crate::count!($($xs)*));
}

#[macro_export]
macro_rules! ingress_join {
    ($key:expr, $total:expr, $msg:expr; $($task:expr $(,)?)+) => {
        {
            let key = $key;
            let msg: Option<&str> = $msg;
            let num_futures = $crate::count!($($task)*);
            let i = $total / num_futures as f64;

            paste::paste! {
                $( let [ <unique_name $task> ] = {
                    {
                        let key = key.clone();
                        let msg = msg.clone();
                        async move {
                            let res = $task.await;
                            if let Some(key) = key {
                                $crate::api::proxy::send::send_ingress(key, i, msg).await?;
                            }
                            res
                        }
                    }
                };)+
            }

            paste::paste! {
                tokio::try_join! (
                    $( [ <unique_name $task>] ),+
                )
            }
        }
    };
}

/// TryStreamExt concurrently for Ingress feeds
#[tracing::instrument(skip(stream, f))]
pub async fn ingress_try_for_each<I, F, Fut, T>(
	stream: I,
	limit: Option<usize>,
	key: Option<&IngressId>,
	total: f64,
	num_futs: usize,
	message: Option<&str>,
	f: F,
) -> crate::Result<()>
where
	I: futures::TryStreamExt<Error = crate::Error> + TryStream<Ok = T>,
	F: FnMut(T) -> Fut + Send,
	Fut: Future<Output = crate::Result<()>> + Send,
	T: Send,
{
	let mut f = f;
	stream
		.try_for_each_concurrent(limit, |item| {
			let f = f(item);
			async move {
				f.await?;
				if let Some(key) = key {
					send_ingress(key, total / (num_futs as f64), message).await?;
				}

				Ok(())
			}
		})
		.await
}
