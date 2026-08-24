use super::command::DebugTaskCommand;
use super::dns::DebugTaskDns;
use super::http::DebugTaskHttp;
use super::task::{debug_panic_error, run_debug_task};
use super::{DebugBundleSide, try_write_json_file};
use crate::constants::DEFAULT_API_DOMAIN;
use camino::Utf8Path;
use futures::future::{BoxFuture, join_all};
use serde::Serialize;
use std::error::Error;
use std::net::IpAddr;

pub async fn populate_debug_tasks(dir: &Utf8Path, side: DebugBundleSide, backend_addrs: Vec<IpAddr>) {
    #[cfg(target_os = "linux")]
    let fwmark = match side {
        DebugBundleSide::Ui => None,
        DebugBundleSide::Service => Some(crate::net::FWMARK),
    };
    #[cfg(not(target_os = "linux"))]
    let fwmark = None;
    let mut tasks: Vec<BoxFuture<'_, ()>> = Vec::new();
    for (name, target) in [
        ("ping-cloudflare-ipv4", "1.1.1.1"),
        ("ping-google-ipv4", "8.8.8.8"),
        ("ping-cloudflare-ipv6", "2606:4700:4700::1111"),
        ("ping-google-ipv6", "2001:4860:4860::8888"),
    ] {
        let (program, args) = ping_command(target, fwmark);
        add_task(&mut tasks, dir, side, name, DebugTaskCommand::run(program, args));
    }
    for (name, host) in [
        ("dns-apple", "www.apple.com"),
        ("dns-google", "google.com"),
        ("dns-obscura-backend", DEFAULT_API_DOMAIN),
    ] {
        add_task(&mut tasks, dir, side, name, DebugTaskDns::run(host));
    }
    for (name, url, addrs) in [
        ("http-apple", "https://www.apple.com/robots.txt", None),
        ("http-google", "https://google.com/robots.txt", None),
        (
            "http-obscura-backend",
            "https://v1.api.prod.obscura.net/api/ping",
            Some(backend_addrs.clone()),
        ),
    ] {
        for sni in [true, false] {
            let name = if sni { name.to_owned() } else { format!("{name}-nosni") };
            add_task(&mut tasks, dir, side, &name, DebugTaskHttp::run(url, addrs.clone(), sni, fwmark));
        }
    }
    for (name, url) in [
        ("http-obscura-backend-sni-apple", "https://apple.com/api/ping"),
        ("http-obscura-backend-sni-google", "https://google.com/api/ping"),
    ] {
        add_task(
            &mut tasks,
            dir,
            side,
            name,
            DebugTaskHttp::run(url, Some(backend_addrs.clone()), true, fwmark),
        );
    }
    join_all(tasks).await;
    tracing::info!(message_id = "bF6nWd4Q", %dir, "debug tasks finished");
}

fn ping_command(target: &str, fwmark: Option<u32>) -> (String, Vec<String>) {
    #[cfg(target_os = "windows")]
    let (program, args) = ("ping", ["-n", "2", "-w", "2000", target]);
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let (program, args) = (if target.contains(':') { "ping6" } else { "ping" }, ["-o", "-c", "2", target]);
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    let (program, args) = ("ping", ["-c", "2", "-W", "2", target]);
    let mut args = args.map(String::from).to_vec();
    if let Some(fwmark) = fwmark {
        args.extend(["-m".to_owned(), fwmark.to_string()]);
    }
    (program.to_owned(), args)
}

pub(crate) fn add_task<T>(
    tasks: &mut Vec<BoxFuture<'_, ()>>,
    dir: &Utf8Path,
    side: DebugBundleSide,
    name: &str,
    task: impl Future<Output = Result<T, Box<dyn Error>>> + Send + 'static,
) where
    T: Serialize + Send + Sync + 'static,
{
    let path = dir.join(format!("{name}-{}.task.json", side.as_str()));
    tasks.push(Box::pin(async move {
        let task = tokio::spawn(run_debug_task(task)).await.unwrap_or_else(debug_panic_error);
        try_write_json_file(path, &task).await;
    }));
}
