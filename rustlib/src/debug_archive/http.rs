use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use crate::debug_archive::dns::DnsTask;
use crate::debug_archive::task::DebugTask;
use crate::debug_archive::task::run_debug_task;
use reqwest::dns::Resolve;
use reqwest::dns::Resolving;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpTask {
    body: Option<String>,
    error: Option<String>,
    header_content_type: Option<String>,
    header_date: Option<String>,
    http_version: Option<String>,
    status_code: Option<u16>,
}

pub async fn debug_http(url: &'static str, dns: Option<DnsTask>, sni: bool) -> DebugTask<HttpTask> {
    run_debug_task(async {
        let mut result = HttpTask {
            body: None,
            error: None,
            header_content_type: None,
            header_date: None,
            http_version: None,
            status_code: None,
        };

        let dns = dns.ok_or("No DNS available.")?;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .dns_resolver(Arc::new(FixedResolver(dns.addrs)))
            .min_tls_version(reqwest::tls::Version::TLS_1_0)
            .timeout(Duration::from_secs(55))
            .tls_sni(sni)
            .build()?;

        let res = match client.get(url).send().await {
            Ok(r) => r,
            Err(err) => {
                result.error = Some(err.to_string());
                return Ok(result);
            }
        };
        result.http_version = Some(format!("{:?}", res.version()));
        result.status_code = Some(res.status().as_u16());

        // TODO: Get certificate info. Reqwest doesn't seem to make this readily available.
        // This probably isn't a big deal because if there is a mismatch the regular logs would make it clear but it would be interesting to see what cert we get.

        let headers = res.headers();
        let header_str = |name| headers.get(name)?.to_str().ok().map(|s| s.to_string());
        result.header_content_type = header_str(reqwest::header::CONTENT_TYPE);
        result.header_date = header_str(reqwest::header::DATE);

        result.body = Some(match res.text().await {
            Ok(t) => t,
            Err(err) => {
                result.error = Some(err.to_string());
                return Ok(result);
            }
        });

        Ok(result)
    })
    .await
}

struct FixedResolver(Vec<IpAddr>);

impl Resolve for FixedResolver {
    fn resolve(&self, _: reqwest::dns::Name) -> Resolving {
        let ips = self.0.clone();
        Box::pin(async move { Ok(Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0))) as _) })
    }
}
