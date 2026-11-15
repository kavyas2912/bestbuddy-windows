use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
    Network,
    Location,
    Account,
    Contacts,
    Messages,
    Browser,
    Files,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub icon: String,
    pub capabilities: Vec<String>,
    pub permissions: Vec<Permission>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    pub capability: String,
    pub params: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub plugin_id: String,
    pub capability: String,
    pub text: String,
    pub data: serde_json::Value,
}
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("plugin error: {0}")]
    Failed(String),
}
#[async_trait]
pub trait BuddyPlugin: Send + Sync {
    fn manifest(&self) -> PluginManifest;
    async fn initialize(&self) -> Result<(), PluginError> {
        Ok(())
    }
    async fn execute(&self, request: PluginRequest) -> Result<PluginResponse, PluginError>;
}
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn BuddyPlugin>>,
    capability_map: HashMap<String, String>,
}
impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            capability_map: HashMap::new(),
        }
    }
    pub async fn register(&mut self, plugin: Box<dyn BuddyPlugin>) -> Result<(), PluginError> {
        let m = plugin.manifest();
        plugin.initialize().await?;
        for c in &m.capabilities {
            self.capability_map.insert(c.clone(), m.id.clone());
        }
        self.plugins.insert(m.id.clone(), plugin);
        Ok(())
    }
    pub fn manifests(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|p| p.manifest()).collect()
    }
    pub fn has_capability(&self, c: &str) -> bool {
        self.capability_map.contains_key(c)
    }
    pub async fn execute(&self, r: PluginRequest) -> Result<PluginResponse, PluginError> {
        let id = self
            .capability_map
            .get(&r.capability)
            .ok_or_else(|| PluginError::Unsupported(r.capability.clone()))?;
        self.plugins
            .get(id)
            .ok_or_else(|| PluginError::Failed(format!("missing plugin {id}")))?
            .execute(r)
            .await
    }
}
impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
