use async_trait::async_trait;
use buddy_plugin::{BuddyPlugin, PluginError, PluginManifest, PluginRequest, PluginResponse};

pub struct FunFactsPlugin;
impl FunFactsPlugin {
    pub fn new() -> Self {
        Self
    }
}

const FACTS: &[(&str, &str)] = &[
    ("engineering", "A suspension bridge can visibly move in the wind while remaining within its designed operating range."),
    ("space", "A day on Venus is longer than its year: Venus rotates once in about 243 Earth days and orbits the Sun in about 225."),
    ("technology", "The first computer mouse was demonstrated in the 1960s and was made of wood."),
    ("music", "The word melody comes from the Greek melōidía, meaning singing or song."),
    ("general", "Honey can remain edible for extremely long periods when sealed and stored properly."),
];

#[async_trait]
impl BuddyPlugin for FunFactsPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "fun-facts".into(),
            name: "Fun & Facts".into(),
            version: "0.6.12".into(),
            icon: "fun-facts".into(),
            capabilities: vec!["funfact.next".into(), "chatter.story".into()],
            permissions: vec![],
        }
    }
    async fn execute(&self, r: PluginRequest) -> Result<PluginResponse, PluginError> {
        let topic = r
            .params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_lowercase();
        let item = FACTS
            .iter()
            .find(|(k, _)| topic.contains(k))
            .or_else(|| FACTS.iter().find(|(k, _)| *k == "general"))
            .unwrap();
        let text = format!(
            "Here's a little {} fact: {}",
            if item.0 == "general" { "fun" } else { item.0 },
            item.1
        );
        Ok(PluginResponse {
            plugin_id: "fun-facts".into(),
            capability: r.capability,
            text: text.clone(),
            data: serde_json::json!({"topic": item.0, "fact": item.1}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_plugin::BuddyPlugin;
    #[test]
    fn manifest_exposes_fun_fact_capability() {
        let m = FunFactsPlugin::new().manifest();
        assert_eq!(m.id, "fun-facts");
        assert!(m.capabilities.contains(&"funfact.next".to_string()));
    }
}
