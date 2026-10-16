use async_trait::async_trait;
use buddy_plugin::{
    BuddyPlugin, Permission, PluginError, PluginManifest, PluginRequest, PluginResponse,
};
use serde::Deserialize;
#[derive(Deserialize)]
struct GeoResp {
    results: Option<Vec<Geo>>,
}
#[derive(Deserialize)]
struct Geo {
    name: String,
    latitude: f64,
    longitude: f64,
    admin1: Option<String>,
    country: Option<String>,
}
#[derive(Deserialize)]
struct Forecast {
    current: Current,
    daily: Daily,
    #[serde(default)]
    timezone: Option<String>,
}
#[derive(Deserialize)]
struct Current {
    temperature_2m: f64,
    relative_humidity_2m: f64,
    apparent_temperature: f64,
    weather_code: i32,
    wind_speed_10m: f64,
    time: String,
}
#[derive(Deserialize)]
struct Daily {
    time: Vec<String>,
    weather_code: Vec<i32>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
}
pub struct WeatherPlugin;
impl WeatherPlugin {
    pub fn new() -> Self {
        Self
    }
    fn icon(code: i32) -> &'static str {
        match code {
            0 => "☀️",
            1 | 2 => "🌤️",
            3 => "☁️",
            45 | 48 => "🌫️",
            51..=67 => "🌧️",
            71..=77 => "❄️",
            80..=82 => "🌦️",
            95..=99 => "⛈️",
            _ => "🌤️",
        }
    }
}
#[async_trait]
impl BuddyPlugin for WeatherPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "weather".into(),
            name: "Weather".into(),
            version: "0.6.12".into(),
            icon: "weather".into(),
            capabilities: vec!["weather.current".into(), "weather.forecast".into()],
            permissions: vec![Permission::Network],
        }
    }
    async fn execute(&self, r: PluginRequest) -> Result<PluginResponse, PluginError> {
        let loc = r
            .params
            .get("location")
            .and_then(|v| v.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                PluginError::Failed(
                    "Set a city or town in the profile before fetching weather".into(),
                )
            })?;
        let g=reqwest::Client::builder().timeout(std::time::Duration::from_secs(12)).build().expect("HTTP client").get(format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",urlencoding::encode(loc))).send().await.map_err(|e|PluginError::Failed(e.to_string()))?.json::<GeoResp>().await.map_err(|e|PluginError::Failed(e.to_string()))?;
        let geo = g
            .results
            .and_then(|v| v.into_iter().next())
            .ok_or_else(|| PluginError::Failed(format!("Location not found: {loc}")))?;
        let u=format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min&forecast_days=5&timezone=auto",geo.latitude,geo.longitude);
        let f = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .expect("HTTP client")
            .get(u)
            .send()
            .await
            .map_err(|e| PluginError::Failed(e.to_string()))?
            .json::<Forecast>()
            .await
            .map_err(|e| PluginError::Failed(e.to_string()))?;
        let days=f.daily.time.iter().enumerate().map(|(i,d)|serde_json::json!({"date":d,"icon":Self::icon(f.daily.weather_code[i]),"weather_code":f.daily.weather_code[i],"high":f.daily.temperature_2m_max[i],"low":f.daily.temperature_2m_min[i],"location":geo.name})).collect::<Vec<_>>();
        let c = Self::icon(f.current.weather_code);
        let text=format!("In {}, it is {:.0}°C and {}. It feels like {:.0}°C, humidity is {:.0}%, with wind around {:.0} km/h.",geo.name,f.current.temperature_2m,c,f.current.apparent_temperature,f.current.relative_humidity_2m,f.current.wind_speed_10m);
        Ok(PluginResponse {
            plugin_id: "weather".into(),
            capability: r.capability,
            text,
            data: serde_json::json!({"location":geo.name,"region":geo.admin1,"country":geo.country,"current":{"temperature":f.current.temperature_2m,"feels_like":f.current.apparent_temperature,"humidity":f.current.relative_humidity_2m,"wind":f.current.wind_speed_10m,"icon":c,"weather_code":f.current.weather_code,"local_time":f.current.time},"timezone":f.timezone,"forecast":days}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_plugin::BuddyPlugin;
    #[test]
    fn manifest_exposes_weather_capabilities() {
        let m = WeatherPlugin::new().manifest();
        assert_eq!(m.id, "weather");
        assert!(m.capabilities.contains(&"weather.current".to_string()));
        assert!(m.capabilities.contains(&"weather.forecast".to_string()));
    }
}
