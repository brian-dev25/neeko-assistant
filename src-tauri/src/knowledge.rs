use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnowledgeFact {
    pub id: String,
    pub category: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_now")]
    pub created: String,
}

fn default_source() -> String {
    "chat".to_string()
}

fn default_now() -> String {
    chrono_now()
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_millis()))
        .unwrap_or_else(|_| "0".to_string())
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct KnowledgeStore {
    pub facts: Vec<KnowledgeFact>,
}

fn knowledge_path() -> Option<PathBuf> {
    let dir = dirs::config_dir()
        .or_else(|| dirs::data_dir())
        .or_else(|| dirs::home_dir())?;
    Some(dir.join("neeko-assistant").join("knowledge.json"))
}

fn load_store() -> KnowledgeStore {
    let path = match knowledge_path() {
        Some(p) => p,
        None => return KnowledgeStore::default(),
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_store(store: &KnowledgeStore) -> Result<(), String> {
    let path = knowledge_path().ok_or("No se pudo determinar ruta de knowledge")?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        &path,
        serde_json::to_string_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn list_facts() -> Vec<KnowledgeFact> {
    load_store().facts
}

pub fn add_fact(category: &str, key: &str, value: &str, source: &str) -> KnowledgeFact {
    let mut store = load_store();

    if category.eq_ignore_ascii_case("preferencia") {
        if let Some(existing) = store.facts.iter().find(|f| {
            f.category.eq_ignore_ascii_case(category)
                && f.key.eq_ignore_ascii_case(key)
                && f.value.eq_ignore_ascii_case(value)
        }) {
            return existing.clone();
        }
    }

    // Preferencias permiten varias entradas con la misma clave ("me gusta").
    // El resto de categorias actualiza por key+category para mantener datos unicos como CPU/GPU.
    let existing_id = store
        .facts
        .iter()
        .find(|f| {
            !category.eq_ignore_ascii_case("preferencia")
                && f.category.eq_ignore_ascii_case(category)
                && f.key.eq_ignore_ascii_case(key)
        })
        .map(|f| f.id.clone());

    if let Some(id) = existing_id {
        if let Some(existing) = store.facts.iter_mut().find(|f| f.id == id) {
            existing.value = value.to_string();
            existing.source = source.to_string();
            let fact = existing.clone();
            let _ = save_store(&store);
            return fact;
        }
    }

    let fact = KnowledgeFact {
        id: format!("k_{}", chrono_now()),
        category: category.to_string(),
        key: key.to_string(),
        value: value.to_string(),
        source: source.to_string(),
        created: chrono_now(),
    };
    store.facts.push(fact.clone());
    let _ = save_store(&store);
    fact
}

pub fn delete_fact(id: &str) -> bool {
    let mut store = load_store();
    let before = store.facts.len();
    store.facts.retain(|f| f.id != id);
    if store.facts.len() < before {
        let _ = save_store(&store);
        true
    } else {
        false
    }
}

pub fn search_facts(query: &str) -> Vec<KnowledgeFact> {
    let store = load_store();
    let q = query.to_lowercase();
    store
        .facts
        .into_iter()
        .filter(|f| {
            f.key.to_lowercase().contains(&q)
                || f.value.to_lowercase().contains(&q)
                || f.category.to_lowercase().contains(&q)
        })
        .collect()
}

pub fn clear_all() -> bool {
    let store = KnowledgeStore::default();
    let _ = save_store(&store);
    true
}

pub fn export_json() -> Result<String, String> {
    let store = load_store();
    serde_json::to_string_pretty(&store).map_err(|e| e.to_string())
}

pub fn import_json(json: &str) -> Result<usize, String> {
    let imported: KnowledgeStore =
        serde_json::from_str(json).map_err(|e| format!("JSON invalido: {}", e))?;
    let mut store = load_store();
    let count = imported.facts.len();
    for fact in imported.facts {
        // Evitar duplicados
        if !store
            .facts
            .iter()
            .any(|f| f.category == fact.category && f.key.eq_ignore_ascii_case(&fact.key))
        {
            store.facts.push(fact);
        }
    }
    save_store(&store)?;
    Ok(count)
}
