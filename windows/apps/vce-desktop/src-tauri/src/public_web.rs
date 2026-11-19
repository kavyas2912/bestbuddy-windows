//! Bounded public HTTPS text fetching. DNS is checked and pinned at every redirect.
use reqwest::{redirect::Policy, Client, Url};
use std::{
    net::{IpAddr, ToSocketAddrs},
    time::Duration,
};

fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !v.is_private()
                && !v.is_loopback()
                && !v.is_link_local()
                && !v.is_multicast()
                && !v.is_unspecified()
                && o[0] != 0
                && o[0] < 224
                && !(o[0] == 100 && (64..=127).contains(&o[1]))
                && !(o[0] == 198 && (18..=19).contains(&o[1]))
                && !(o[0] == 192 && o[1] == 0 && (o[2] == 0 || o[2] == 2))
                && !(o[0] == 198 && o[1] == 51 && o[2] == 100)
                && !(o[0] == 203 && o[1] == 0 && o[2] == 113)
        }
        IpAddr::V6(v) => v
            .to_ipv4_mapped()
            .map(|v| public_address(IpAddr::V4(v)))
            .unwrap_or_else(|| {
                (v.segments()[0] & 0xe000) == 0x2000
                    && !(v.segments()[0] == 0x2001 && v.segments()[1] == 0x0db8)
            }),
    }
}

pub async fn fetch(address: &str) -> Result<String, String> {
    let mut url = Url::parse(address).map_err(|e| e.to_string())?;
    for _ in 0..6 {
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err("Use public HTTPS sources without embedded credentials".into());
        }
        let host = url.host_str().ok_or("Source has no host")?.to_owned();
        if !host.contains('.') && host.parse::<IpAddr>().is_err() {
            return Err("A public source host is required".into());
        }
        let port = url.port_or_known_default().unwrap_or(443);
        let lookup = host.clone();
        let addresses = tauri::async_runtime::spawn_blocking(move || {
            (lookup.as_str(), port)
                .to_socket_addrs()
                .map(|addresses| addresses.collect::<Vec<_>>())
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        if addresses.is_empty()
            || addresses
                .iter()
                .any(|address| !public_address(address.ip()))
        {
            return Err("Private-network sources are not fetched by web research".into());
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(18))
            .resolve_to_addrs(&host, &addresses)
            .user_agent("BestBuddy/1.0 personal research reader")
            .build()
            .map_err(|e| e.to_string())?;
        let mut response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if response.status().is_redirection() {
            let next = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("Redirect has no readable destination")?;
            url = url.join(next).map_err(|e| e.to_string())?;
            continue;
        }
        response = response.error_for_status().map_err(|e| e.to_string())?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if body.len() + chunk.len() > 4_000_000 {
                return Err("Source exceeds the reading limit".into());
            }
            body.extend_from_slice(&chunk);
        }
        return Ok(String::from_utf8_lossy(&body).into_owned());
    }
    Err("Source redirected too many times".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_and_reserved_destinations_are_blocked() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
        ] {
            assert!(!public_address(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_address("8.8.8.8".parse().unwrap()));
        assert!(public_address("2606:4700:4700::1111".parse().unwrap()));
    }
}
