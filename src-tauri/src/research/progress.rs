use crate::research::types::{ProgressEvent, ResearchPhase};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Estado de progreso para una request específica.
#[derive(Clone, Debug, Serialize)]
pub struct ProgressState {
    pub request_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub phase: ResearchPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    pub completed_operations: usize,
    pub updated_at: u64,
}

/// Servicio global de progreso.
pub struct ProgressService {
    states: Mutex<HashMap<String, ProgressState>>,
}

impl ProgressService {
    pub fn global() -> &'static ProgressService {
        static INSTANCE: OnceLock<ProgressService> = OnceLock::new();
        INSTANCE.get_or_init(|| ProgressService {
            states: Mutex::new(HashMap::new()),
        })
    }

    /// Actualiza el estado de progreso para una request.
    pub fn update(&self, event: ProgressEvent) {
        let key = format!("{}:{}", event.request_id, event.generation);
        let state = ProgressState {
            request_id: event.request_id,
            generation: event.generation,
            sequence: event.sequence,
            phase: event.phase,
            domain: event.domain,
            completed_operations: event.completed_operations,
            updated_at: now_millis(),
        };
        if let Ok(mut states) = self.states.lock() {
            // Only update if sequence is newer or same request
            if let Some(existing) = states.get(&key) {
                if event.sequence < existing.sequence {
                    return;
                }
            }
            states.insert(key, state);
        }
    }

    /// Consulta el estado actual de progreso para una request.
    pub fn get(&self, request_id: &str, generation: u64) -> Option<ProgressState> {
        let key = format!("{}:{}", request_id, generation);
        self.states.lock().ok()?.get(&key).cloned()
    }

    /// Consulta el estado más reciente para un request_id (cualquier generación).
    pub fn get_latest(&self, request_id: &str) -> Option<ProgressState> {
        self.states
            .lock()
            .ok()?
            .values()
            .filter(|s| s.request_id == request_id)
            .max_by_key(|s| s.sequence)
            .cloned()
    }

    /// Limpia estados antiguos (TTL 5 minutos).
    pub fn cleanup(&self) {
        let cutoff = now_millis().saturating_sub(5 * 60 * 1000);
        if let Ok(mut states) = self.states.lock() {
            states.retain(|_, state| state.updated_at > cutoff);
        }
    }

    /// Crea un callback de progreso que notifica al servicio.
    pub fn callback(_request_id: String, _generation: u64) -> Box<dyn Fn(ProgressEvent) + Send + Sync> {
        Box::new(move |event: ProgressEvent| {
            let service = ProgressService::global();
            service.update(event);
        })
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Tauri command: consultar progreso de investigación.
#[tauri::command]
pub fn research_progress(
    request_id: String,
    generation: Option<u64>,
) -> Option<ProgressState> {
    if !crate::config::AppConfig::load().source_research_enabled {
        return None;
    }
    let service = ProgressService::global();
    service.cleanup();
    if let Some(gen) = generation {
        service.get(&request_id, gen)
    } else {
        service.get_latest(&request_id)
    }
}
