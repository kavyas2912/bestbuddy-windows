use serde_json::{json, Value};
use std::{
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
static CACHE: OnceLock<Mutex<Option<(Instant, Value)>>> = OnceLock::new();
fn bundled() -> Value {
    let mut models: Vec<Value> = serde_json::from_str(include_str!("compact_models.json"))
        .expect("bundled compatible model catalog");
    models.retain(|m| {
        m["size_bytes"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= crate::local_services::model_limit())
    });
    models.sort_by_key(|m| std::cmp::Reverse(m["downloads"].as_u64().unwrap_or(0)));
    json!({"source":"Verified Hugging Face GGUF files","ranking":"Repository downloads; popularity, not quality","fetched_at":"2026-10-20","live":false,"runtime_available":!cfg!(target_os="android"),"models":models,"max_model_bytes":crate::local_services::model_limit(),"local_scan_supported":!cfg!(target_os="android")})
}
/// Cached metadata can never replace pinned URLs, hashes or the compatible model allowlist.
fn merge_metadata(data: &mut Value, cached: &Value) {
    for model in data["models"].as_array_mut().unwrap() {
        if let Some(old) = cached["models"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == model["id"]))
        {
            for key in [
                "downloads",
                "availability",
                "policy_checked_at",
                "repo_license",
                "gated",
            ] {
                if !old[key].is_null() {
                    model[key] = old[key].clone();
                }
            }
        }
    }
    data["models"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|m| std::cmp::Reverse(m["downloads"].as_u64().unwrap_or(0)));
    if let Some(date) = cached["fetched_at"].as_str() {
        data["fetched_at"] = json!(date);
    }
}
pub async fn catalog_cached(path: &Path, network_allowed: bool) -> Value {
    let mut base = bundled();
    if let Ok(raw) = std::fs::read(path) {
        if let Ok(old) = serde_json::from_slice::<Value>(&raw) {
            merge_metadata(&mut base, &old);
            base["cached"] = json!(true);
        }
    }
    if !network_allowed {
        return base;
    }
    let mut cached = CACHE.get_or_init(|| Mutex::new(None)).lock().await;
    if let Some((when, data)) = &*cached {
        if when.elapsed() < Duration::from_secs(21600) {
            return data.clone();
        }
    }
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    else {
        return base;
    };
    let repos = base["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["repo"].as_str())
        .map(str::to_owned)
        .collect::<std::collections::HashSet<_>>();
    let total = repos.len();
    let mut jobs = tokio::task::JoinSet::new();
    for repo in repos {
        let client = client.clone();
        jobs.spawn(async move {
            let response = client
                .get(format!("https://huggingface.co/api/models/{repo}"))
                .send()
                .await
                .ok()?;
            let code = response.status().as_u16();
            if matches!(code, 401 | 403 | 404) {
                return Some((
                    repo,
                    json!({"availability":if code==404{"missing"}else{"gated"}}),
                ));
            }
            if !response.status().is_success() {
                return None;
            }
            Some((repo, response.json::<Value>().await.ok()?))
        });
    }
    let checked_at = chrono::Utc::now().to_rfc3339();
    let mut count = 0;
    while let Some(job) = jobs.join_next().await {
        if let Ok(Some((repo, info))) = job {
            for model in base["models"].as_array_mut().unwrap() {
                if model["repo"] == repo {
                    if let Some(n) = info["downloads"].as_u64() {
                        model["downloads"] = json!(n);
                    }
                    model["gated"] = info["gated"].clone();
                    model["availability"] =
                        info.get("availability").cloned().unwrap_or_else(|| {
                            json!(if info["gated"] == false || info["gated"].is_null() {
                                "public"
                            } else {
                                "gated"
                            })
                        });
                    model["policy_checked_at"] = json!(checked_at);
                    model["repo_license"] = info["cardData"]["license"].clone();
                }
            }
            count += 1;
        }
    }
    base["models"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|m| std::cmp::Reverse(m["downloads"].as_u64().unwrap_or(0)));
    base["checked_repositories"] = json!(count);
    base["total_repositories"] = json!(total);
    base["live"] = json!(count == total);
    if count > 0 {
        base["fetched_at"] = json!(checked_at);
        if let Ok(raw) = serde_json::to_vec_pretty(&base) {
            let _ = std::fs::write(path, raw);
        }
    }
    *cached = Some((Instant::now(), base.clone()));
    base
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_sizes_and_hashes() {
        for m in bundled()["models"].as_array().unwrap() {
            assert!(m["size_bytes"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= crate::local_services::model_limit()));
            assert_eq!(m["sha256"].as_str().unwrap().len(), 64);
            assert!(crate::llm_assets::model_asset(m["id"].as_str().unwrap()).is_some());
        }
    }
    #[test]
    fn metadata_cannot_redirect_downloads() {
        let mut base = bundled();
        let old_hash = base["models"][0]["sha256"].clone();
        let id = base["models"][0]["id"].clone();
        merge_metadata(
            &mut base,
            &json!({"models":[{"id":id,"downloads":9999999999u64,"availability":"gated","sha256":"tampered","url":"https://untrusted.invalid/model"}]}),
        );
        assert_eq!(base["models"][0]["sha256"], old_hash);
        assert_eq!(base["models"][0]["availability"], "gated");
    }
}
