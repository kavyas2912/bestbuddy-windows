use serde_json::{json, Value};
use std::sync::OnceLock;
pub fn registry() -> &'static Value {
    static REGISTRY: OnceLock<Value> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        serde_json::from_str(include_str!("../source_registry.json"))
            .expect("bundled source registry")
    })
}
pub fn geography() -> &'static Value {
    static GEOGRAPHY: OnceLock<Value> = OnceLock::new();
    GEOGRAPHY.get_or_init(|| {
        serde_json::from_str(include_str!("../geography.json")).expect("news geography")
    })
}

pub fn canonical_country(country: &str) -> String {
    let raw = country.trim().to_lowercase();
    geography()["aliases"][&raw]
        .as_str()
        .unwrap_or(&raw)
        .to_owned()
}

pub fn explicit_location(city: &str, country: &str) -> (String, String) {
    let parts: Vec<_> = city
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let named = parts.last().map(|part| canonical_country(part));
    let country = named
        .filter(|name| geography()["countries"][name].is_string())
        .unwrap_or_else(|| canonical_country(country));
    (parts.first().unwrap_or(&city).to_string(), country)
}

pub fn location_rule(city: &str, country: &str) -> &'static Value {
    let city = city.split(',').next().unwrap_or(city).trim().to_lowercase();
    &geography()["cities"][format!("{}|{city}", canonical_country(country))]
}

pub fn coverage_region(city: &str, region: &str, country: &str) -> String {
    if let Some(region) = location_rule(city, country)["region"].as_str() {
        return region.into();
    }
    if canonical_country(country) == "united states" {
        if let Some(region) = geography()["region_aliases"][region.trim().to_lowercase()].as_str() {
            return region.into();
        }
    }
    region.trim().into()
}

fn scoped_publishers(
    category: &str,
    city: &str,
    region: &str,
    country: &str,
) -> Option<Vec<String>> {
    let values = match category {
        "local" => &location_rule(city, country)["local"],
        "regional" => {
            let rule = location_rule(city, country);
            if rule["regional"].is_array() {
                &rule["regional"]
            } else if canonical_country(country) == "united states"
                && coverage_region(city, region, country) == "New England"
            {
                &geography()["cities"]["united states|boston"]["regional"]
            } else {
                return None;
            }
        }
        "world" | "national" | "breaking" | "political" => {
            &geography()["national"][canonical_country(country)]
        }
        _ => return None,
    };
    values.as_array().map(|rows| {
        rows.iter()
            .filter_map(|id| id.as_str().map(str::to_owned))
            .collect()
    })
}

pub fn city_state(city: &str, country: &str) -> Option<String> {
    if let Some(state) = location_rule(city, country)["state"].as_str() {
        return Some(state.into());
    }
    if canonical_country(country) != "india" {
        return None;
    }
    registry()["india_city_states"][city.split(',').next().unwrap_or(city).trim().to_lowercase()]
        .as_str()
        .map(str::to_owned)
}
pub fn language_code(language: &str) -> &str {
    match language
        .trim()
        .to_lowercase()
        .split(['-', '_'])
        .next()
        .unwrap_or("")
    {
        "hindi" | "hi" => "hi",
        "marathi" | "mr" => "mr",
        "tamil" | "ta" => "ta",
        "telugu" | "te" => "te",
        "bengali" | "bn" => "bn",
        "malayalam" | "ml" => "ml",
        "kannada" | "kn" => "kn",
        "gujarati" | "gu" => "gu",
        "punjabi" | "pa" => "pa",
        "french" | "fr" => "fr",
        "german" | "de" => "de",
        "spanish" | "es" => "es",
        _ => "en",
    }
}
pub fn candidates(category: &str, city: &str, region: &str, country: &str) -> Vec<String> {
    candidates_for_language(category, city, region, country, "en")
}
pub fn candidates_for_language(
    category: &str,
    city: &str,
    region: &str,
    country: &str,
    language: &str,
) -> Vec<String> {
    let key = canonical_country(country);
    if let Some(ids) = scoped_publishers(category, city, region, &key) {
        return ids
            .into_iter()
            .filter(|id| registry()["publishers"][id]["domain"].is_string())
            .collect();
    }
    let inferred = city_state(city, country);
    let region = if region.trim().is_empty() {
        inferred.as_deref().unwrap_or(region)
    } else {
        region
    };
    let mut ids = if key == "india" && registry()["india_categories"][category].is_array() {
        registry()["india_categories"][category]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    } else if matches!(category, "business" | "business-national") {
        registry()["financial"][&key]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_else(|| {
                super::recommended_sources(category, country, region)
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            })
    } else if !matches!(category, "world" | "business-world" | "science" | "ai-tech")
        && registry()["national"][&key].is_array()
    {
        registry()["national"][&key]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    } else {
        super::recommended_sources(category, country, region)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if matches!(category, "local" | "regional") {
        if key == "india" {
            if let Some(rows) = registry()["regions"][region.trim().to_lowercase()].as_array() {
                let mut regional = rows
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                regional.extend(ids);
                ids = regional;
            }
        }
        ids.sort_by_key(|id| match id.as_str() {
            "google" | "msn" => 3,
            "bbc" | "reuters" | "ap" => 2,
            "express" | "hindu" | "ndtv" | "toi" | "hindustantimes" | "indiatoday" => 1,
            _ => 0,
        });
    }
    if !matches!(
        category,
        "local" | "regional" | "national" | "world" | "breaking" | "political"
    ) {
        ids.extend(["google".into(), "msn".into()]);
    }
    ids.retain(|id| {
        !matches!(
            category,
            "local" | "regional" | "national" | "world" | "breaking" | "political"
        ) || !matches!(id.as_str(), "google" | "msn")
    });
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()) && registry()["publishers"][id]["name"].is_string());
    let wanted = language_code(language);
    ids.retain(|id| {
        let lang = registry()["publishers"][id]["language"]
            .as_str()
            .unwrap_or("en");
        lang == "en" || lang == wanted
    });
    ids
}
pub fn catalog(category: &str, city: &str, region: &str, country: &str) -> Value {
    let inferred = city_state(city, country);
    let region = if region.trim().is_empty() {
        inferred.as_deref().unwrap_or(region)
    } else {
        region
    };
    let scope = match category {
        "world" | "business-world" => "Worldwide".to_string(),
        "science" | "ai-tech" => "Global research".to_string(),
        "local" => format!("{city}, {region}, {country}"),
        "regional" => format!("{region}, {country}"),
        _ => country.to_string(),
    };
    let sources=candidates(category,city,region,country).into_iter().map(|id|json!({"id":id,"name":registry()["publishers"][&id]["name"],"domain":registry()["publishers"][&id]["domain"],"scope":scope,"status":"not_checked","transport":if id=="google"{"Google News RSS"}else if id=="msn"{"Google News indexed MSN"}else{"Publisher RSS or scoped publisher search"}})).collect::<Vec<_>>();
    json!({"category":category,"scope":scope,"city":city,"region":region,"country":country,"sources":sources,"curated_by":"Bundled publisher registry and background feed checks"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boston_publishers_change_with_scope_and_country() {
        let (city, country) = explicit_location("Boston, MA, USA", "India");
        assert_eq!(city, "Boston");
        assert_eq!(country, "united states");
        assert_eq!(
            city_state(&city, &country).as_deref(),
            Some("Massachusetts")
        );
        let local = candidates("local", &city, "Massachusetts", &country);
        assert!(local.contains(&"wbur".into()));
        assert!(!local.contains(&"hindu".into()));
        let regional = candidates("regional", &city, "New England", &country);
        assert!(regional.contains(&"nhpr".into()));
        for scope in ["national", "world"] {
            let rows = candidates(scope, &city, "", &country);
            assert!(rows.contains(&"npr".into()));
            assert!(!rows.contains(&"bbc".into()));
            assert!(!rows.contains(&"hindu".into()));
            assert!(!rows.contains(&"google".into()));
        }
        assert!(candidates("national", "Ahmedabad", "Gujarat", "IN").contains(&"hindu".into()));
    }
    #[test]
    fn topic_and_location_change_candidates() {
        assert_eq!(city_state("Pune", "India").as_deref(), Some("Maharashtra"));
        assert!(!candidates("local", "Pune", "", "India").contains(&"loksatta".to_string()));
        assert!(
            candidates_for_language("local", "Pune", "", "India", "Marathi")
                .contains(&"loksatta".to_string())
        );
        assert!(
            !candidates_for_language("local", "Pune", "", "India", "Hindi")
                .contains(&"loksatta".to_string())
        );
        assert!(candidates("business", "Pune", "", "India").contains(&"cnbctv18".into()));
        assert!(!candidates("science", "Pune", "", "India").contains(&"cnbctv18".into()));
        assert!(candidates("business", "London", "", "UK").contains(&"ft".into()));
    }
}

pub fn place_names(place: &str) -> Vec<String> {
    let place = place.trim().to_lowercase();
    let mut result = vec![place.clone()];
    if let Some(aliases) = registry()["place_aliases"][&place].as_array() {
        result.extend(
            aliases
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned)),
        );
    }
    result
}
