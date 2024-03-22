use async_std::task::sleep;
use std::time::{Duration, Instant};

#[cfg(test)]
mod nitro;

async fn wait_for_condition<F, Fut>(condition: F, interval: Duration, timeout: Duration) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = Instant::now();

    while Instant::now().duration_since(start) < timeout {
        if condition().await {
            return true;
        }
        sleep(interval).await;
    }
    false
}
