use futures::TryStreamExt as _;
use reqwest::dns::{Resolve, Resolving};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt as _;
use tokio_util::io::StreamReader;

const BODY_LIMIT: usize = 16 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugTaskHttp {
    addrs: Option<Vec<IpAddr>>,
    body: Option<String>,
    body_truncated: bool,
    error: Option<String>,
    fwmark: Option<u32>,
    header_content_type: Option<String>,
    header_date: Option<String>,
    http_version: Option<String>,
    sni: bool,
    status_code: Option<u16>,
    url: String,
}

impl DebugTaskHttp {
    pub async fn run(url: &'static str, addrs: Option<Vec<IpAddr>>, sni: bool, fwmark: Option<u32>) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(addrs) = &addrs
            && addrs.is_empty()
        {
            return Err("no known addresses".into());
        }
        let mut result = Self {
            addrs: addrs.clone(),
            body: None,
            body_truncated: false,
            error: None,
            fwmark,
            header_content_type: None,
            header_date: None,
            http_version: None,
            sni,
            status_code: None,
            url: url.to_owned(),
        };

        let builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .min_tls_version(reqwest::tls::Version::TLS_1_0)
            .timeout(Duration::from_secs(55))
            .tls_sni(sni);
        let builder = match addrs {
            Some(addrs) => builder.dns_resolver(Arc::new(FixedResolver(addrs))),
            None => builder,
        };
        #[cfg(target_os = "linux")]
        let builder = builder.so_mark(fwmark);
        let client = builder.build()?;

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

        let mut reader = StreamReader::new(res.bytes_stream().map_err(std::io::Error::other));
        let mut body = Vec::new();
        if let Err(error) = (&mut reader).take(u64::try_from(BODY_LIMIT + 1)?).read_to_end(&mut body).await {
            result.error = Some(error.to_string());
        }
        result.body_truncated = body.len() > BODY_LIMIT;
        body.truncate(BODY_LIMIT);
        result.body = Some(String::from_utf8_lossy(&body).into_owned());

        Ok(result)
    }
}

struct FixedResolver(Vec<IpAddr>);

impl Resolve for FixedResolver {
    fn resolve(&self, _: reqwest::dns::Name) -> Resolving {
        let ips = self.0.clone();
        Box::pin(async move {
            let addrs: reqwest::dns::Addrs = Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}
