use async_std::task::sleep;
use std::{
    process::Output,
    time::{Duration, Instant},
};

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

fn assert_output_is_receipt(output: Output) -> bool {
    if !output.stderr.is_empty() {
        panic!("got error output: {:?}", String::from_utf8(output.stderr));
    }
    let stdout = String::from_utf8(output.stdout).unwrap();
    stdout.trim().starts_with("TransactionReceipt")
}
