use heck::ToSnakeCase;
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ── Identificadores estables ────────────────────────────────────────────────

/// ID de fuente estable asignado por backend, nunca por el modelo.
/// Formato: `s1`, `s2`, etc. Se asignan al descubrir y nunca se reasignan.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub String);

impl SourceId {
    pub fn new(n: usize) -> Self {
        Self(format!("s{n}"))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn parse_id(s: &str) -> Option<Self> {
        s.strip_prefix('s').and_then(|n| n.parse::<usize>().ok()).map(|_| Self(s.to_string()))
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// ID de fragmento estable: `s2:p4` = fuente s2, fragmento p4.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FragmentId(pub String);

impl FragmentId {
    pub fn new(source: &SourceId, idx: usize) -> Self {
        Self(format!("{}:p{}", source, idx))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FragmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// ID de enlace estable: `s2:l3` = fuente s2, enlace l3.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinkId(pub String);

impl LinkId {
    pub fn new(source: &SourceId, idx: usize) -> Self {
        Self(format!("{}:l{}", source, idx))
    }
    #[allow(dead_code)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LinkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── Intención ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    GeneralFact,
    CurrentVersion,
    Documentation,
    HardwareCapability,
    Troubleshooting,
    UserExperience,
    Comparison,
    News,
    Code,
    Bug,
    Person,
    Opinions,
    Product,
    Verification,
    Unknown,
}

impl Intent {
    pub fn from_model(value: &str) -> Self {
        match value {
            "general" | "fact" => Self::GeneralFact,
            "current" | "version" => Self::CurrentVersion,
            "official" | "docs" => Self::Documentation,
            "hardware" => Self::HardwareCapability,
            "bug" => Self::Troubleshooting,
            "code" | "software" => Self::Code,
            "opinions" | "reviews" => Self::UserExperience,
            "comparison" => Self::Comparison,
            "news" => Self::News,
            "person" => Self::Person,
            "product" => Self::Product,
            "verification" => Self::Verification,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for Intent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::GeneralFact => "general",
            Self::CurrentVersion => "current",
            Self::Documentation => "official",
            Self::HardwareCapability => "hardware",
            Self::Troubleshooting => "bug",
            Self::Code => "code",
            Self::UserExperience | Self::Opinions => "opinions",
            Self::Comparison => "comparison",
            Self::News => "news",
            Self::Person => "person",
            Self::Product => "product",
            Self::Verification => "verification",
            Self::Unknown | Self::Bug => "general",
        };
        write!(f, "{s}")
    }
}

// ── Entidad ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub variants: Vec<String>,
    #[serde(default)]
    pub unverified_relation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Software,
    Hardware,
    Person,
    Organization,
    Concept,
    Other,
}

// ── Pregunta concreta ───────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackedQuestion {
    pub text: String,
    pub status: QuestionStatus,
    #[serde(default)]
    pub evidence_ids: Vec<FragmentId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Pending,
    Answered,
    Contradictory,
    Insufficient,
}

// ── Fragmentos y enlaces ────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fragment {
    pub fragment_id: FragmentId,
    pub source_id: SourceId,
    pub text: String,
    #[serde(default)]
    pub section: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredLink {
    pub link_id: LinkId,
    pub source_id: SourceId,
    pub url: String,
    pub anchor_text: String,
}

// ── Documento fuente ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    SearchSnippet,
    ApiMetadata,
    ContentRead,
    Partial,
    Blocked,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub source_id: SourceId,
    pub title: String,
    pub url: String,
    pub final_url: String,
    pub domain: String,
    pub provider: String,
    pub status: DocumentStatus,
    #[serde(default)]
    pub http_status: Option<u16>,
    pub retrieved_at: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub read_scope: ReadScope,
    pub bytes_downloaded: usize,
    pub text_extracted_chars: usize,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub headers: Vec<Fragment>,
    #[serde(default)]
    pub tables: Vec<Fragment>,
    #[serde(default)]
    pub links: Vec<DiscoveredLink>,
}

impl Document {
    /// Proyecta a `Source` para mantener compatibilidad con `Reply.sources`.
    pub fn to_source(&self) -> crate::research::Source {
        let snippet = self
            .headers
            .iter()
            .chain(self.tables.iter())
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        crate::research::Source {
            source_id: self.source_id.as_str().to_string(),
            title: self.title.clone(),
            url: self.url.clone(),
            domain: self.domain.clone(),
            source_type: provider_label(&self.provider),
            snippet,
            provider: self.provider.clone(),
            retrieved_at: self.retrieved_at.clone(),
            read_status: format!("{:?}", self.status).to_snake_case(),
            published_at: self.published_at.clone(),
        }
    }
}

fn provider_label(provider: &str) -> String {
    match provider {
        "wikipedia" => "Encyclopedia",
        "github" => "Repository",
        "reddit" => "Community",
        "google" => "Web",
        _ => "Web",
    }
    .to_string()
}

// ── Lectura y Fragmentos ────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadScope {
    #[default]
    Full,
    Snippet,
    Focused,
}

// ── Autoridad ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLevel {
    ConfirmedOfficial,
    CandidateOfficial,
    SecondarySource,
    Community,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authority {
    pub source_id: SourceId,
    pub level: AuthorityLevel,
    pub relation_to_entity: String,
    pub supporting_evidence: Vec<FragmentId>,
}

// ── Operación y su resultado ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub op: OperationType,
    pub args: OperationArgs,
    pub result: OperationResult,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    Search,
    Read,
    FollowLink,
    GitHubRead,
    RedditRead,
    Finish,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum OperationArgs {
    Search {
        query: String,
        strategy: String,
        #[serde(default)]
        domains: Vec<String>,
        #[serde(default)]
        freshness: Option<String>,
    },
    Read {
        source_id: SourceId,
        #[serde(default)]
        focus: Option<String>,
    },
    FollowLink {
        source_id: SourceId,
        link_id: LinkId,
        #[serde(default)]
        focus: Option<String>,
    },
    #[serde(rename = "github_read")]
    GitHubRead {
        source_id: SourceId,
        resource: String,
        #[serde(default)]
        filters: Option<String>,
    },
    RedditRead {
        source_id: SourceId,
        #[serde(default)]
        max_comments: Option<usize>,
        #[serde(default)]
        sort: Option<String>,
    },
    Finish {
        status: FinishStatus,
        #[serde(default)]
        pending_questions: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishStatus {
    Complete,
    Partial,
    Insufficient,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationResult {
    Success {
        documents: Vec<Document>,
        #[serde(default)]
        fragments: Vec<Fragment>,
        #[serde(default)]
        links: Vec<DiscoveredLink>,
    },
    Error {
        code: ErrorCode,
        message: String,
        #[serde(default)]
        provider: Option<String>,
    },
}

// ── Errores estructurados ───────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NoResults,
    Blocked,
    Timeout,
    UnsupportedFormat,
    InvalidReference,
    RateLimited,
    BudgetExhausted,
    NetworkError,
    ModelError,
    InvalidResponse,
}

// ── Presupuesto ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Budget {
    pub total_operations: usize,
    pub max_searches: usize,
    pub max_reads: usize,
    pub max_follow_depth: usize,
    pub max_candidates: usize,
    pub max_new_per_search: usize,
    pub max_model_calls: usize,
    pub total_deadline_secs: u64,
    pub writing_deadline_secs: u64,
    pub http_timeout_secs: u64,
    pub model_timeout_secs: u64,
    pub max_document_bytes: usize,
    pub max_evidence_chars: usize,
    pub writing_token_budget: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            total_operations: 8,
            max_searches: 4,
            max_reads: 6,
            max_follow_depth: 3,
            max_candidates: 24,
            max_new_per_search: 6,
            max_model_calls: 8,
            total_deadline_secs: 180,
            writing_deadline_secs: 45,
            http_timeout_secs: 12,
            model_timeout_secs: 45,
            max_document_bytes: 1_048_576,
            max_evidence_chars: 2400,
            writing_token_budget: 1200,
        }
    }
}

impl Budget {
    pub fn deadline(&self) -> Instant {
        Instant::now() + std::time::Duration::from_secs(self.total_deadline_secs)
    }

    #[allow(dead_code)]
    pub fn writing_deadline(&self) -> Instant {
        Instant::now() + std::time::Duration::from_secs(self.writing_deadline_secs)
    }

    pub fn http_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.http_timeout_secs)
    }

    #[allow(dead_code)]
    pub fn remaining_deadline(&self, deadline: Instant) -> std::time::Duration {
        deadline.saturating_duration_since(Instant::now())
    }
}

// ── Contadores ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BudgetCounters {
    pub operations: usize,
    pub searches: usize,
    pub reads: usize,
    pub follow_depth: usize,
    pub candidates: usize,
    pub model_calls: usize,
    pub unproductive_rounds: usize,
}

impl BudgetCounters {
    pub fn can_operate(&self, budget: &Budget) -> bool {
        self.operations < budget.total_operations
            && self.model_calls < budget.max_model_calls
    }

    pub fn can_search(&self, budget: &Budget) -> bool {
        self.can_operate(budget) && self.searches < budget.max_searches
    }

    pub fn can_read(&self, budget: &Budget) -> bool {
        self.can_operate(budget) && self.reads < budget.max_reads
    }

    #[allow(dead_code)]
    pub fn can_follow(&self, budget: &Budget) -> bool {
        self.can_read(budget) && self.follow_depth < budget.max_follow_depth
    }

    pub fn can_add_candidates(&self, budget: &Budget, count: usize) -> bool {
        self.candidates + count <= budget.max_candidates
    }

    pub fn record_operation(&mut self) {
        self.operations += 1;
    }

    pub fn record_search(&mut self) {
        self.searches += 1;
    }

    pub fn record_read(&mut self) {
        self.reads += 1;
    }

    pub fn record_follow(&mut self) {
        self.follow_depth += 1;
    }

    pub fn record_model_call(&mut self) {
        self.model_calls += 1;
    }

    pub fn record_unproductive(&mut self) {
        self.unproductive_rounds += 1;
    }

    pub fn reset_unproductive(&mut self) {
        self.unproductive_rounds = 0;
    }
}

// ── Eventos de progreso ─────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchPhase {
    Planning,
    Searching,
    Reading,
    Checking,
    Writing,
    Complete,
    Cancelled,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub request_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub phase: ResearchPhase,
    #[serde(default)]
    pub domain: Option<String>,
    pub completed_operations: usize,
}

// ── Estado principal ────────────────────────────────────────────────────────

pub struct ResearchState {
    pub request_id: String,
    #[allow(dead_code)]
    pub session_id: String,
    pub generation: u64,
    pub original_question: String,
    pub resolved_question: String,
    pub conversation_context: Vec<serde_json::Value>,
    pub last_tool_feedback: Option<String>,
    pub language: String,
    #[allow(dead_code)]
    pub current_date: String,
    pub intent: Intent,
    pub entities: Vec<Entity>,
    pub questions: Vec<TrackedQuestion>,
    pub documents: Vec<Document>,
    pub fragments: Vec<Fragment>,
    pub links: Vec<DiscoveredLink>,
    #[allow(dead_code)]
    pub authorities: Vec<Authority>,
    #[allow(dead_code)]
    pub operations: Vec<Operation>,
    pub budget: Budget,
    pub counters: BudgetCounters,
    pub deadline: Instant,
    pub cancelled: bool,
    pub sequence: u64,
}

impl ResearchState {
    pub fn new(
        request_id: String,
        session_id: String,
        generation: u64,
        question: String,
        language: String,
        budget: Budget,
    ) -> Self {
        let current_date = chrono::Utc::now()
            .format("%Y-%m-%d")
            .to_string();
        let deadline = budget.deadline();
        Self {
            request_id,
            session_id,
            generation,
            original_question: question.clone(),
            resolved_question: question,
            conversation_context: Vec::new(),
            last_tool_feedback: None,
            language,
            current_date,
            intent: Intent::Unknown,
            entities: Vec::new(),
            questions: Vec::new(),
            documents: Vec::new(),
            fragments: Vec::new(),
            links: Vec::new(),
            authorities: Vec::new(),
            operations: Vec::new(),
            budget,
            counters: BudgetCounters::default(),
            deadline,
            cancelled: false,
            sequence: 0,
        }
    }

    /// Busca un documento por source_id.
    pub fn find_document(&self, id: &SourceId) -> Option<&Document> {
        self.documents.iter().find(|d| &d.source_id == id)
    }

    /// Busca un enlace por link_id dentro de un documento.
    pub fn find_link(&self, source_id: &SourceId, link_id: &LinkId) -> Option<&DiscoveredLink> {
        self.documents
            .iter()
            .find(|d| &d.source_id == source_id)
            .and_then(|d| d.links.iter().find(|l| &l.link_id == link_id))
    }

    /// Añade un documento y devuelve su source_id.
    #[allow(dead_code)]
    pub fn add_document(&mut self, mut doc: Document) -> SourceId {
        let id = SourceId::new(self.documents.len() + 1);
        doc.source_id = id.clone();
        self.documents.push(doc);
        id
    }

    /// Añade fragmentos y los asocia a un documento.
    pub fn add_fragments(&mut self, source_id: &SourceId, texts: Vec<String>, section: Option<String>) -> Vec<FragmentId> {
        let start_idx = self.fragments.len();
        let mut ids = Vec::new();
        for (i, text) in texts.into_iter().enumerate() {
            let fid = FragmentId::new(source_id, start_idx + i);
            self.fragments.push(Fragment {
                fragment_id: fid.clone(),
                source_id: source_id.clone(),
                text,
                section: section.clone(),
            });
            ids.push(fid);
        }
        ids
    }

    /// Añade un enlace descubierto.
    #[allow(dead_code)]
    pub fn add_link(&mut self, link: DiscoveredLink) {
        self.links.push(link);
    }

    #[allow(dead_code)]
    pub fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn has_budget(&self) -> bool {
        self.counters.can_operate(&self.budget)
            && Instant::now() < self.deadline
    }

    pub fn has_search_budget(&self) -> bool {
        self.counters.can_search(&self.budget)
            && Instant::now() < self.deadline
    }

    pub fn has_read_budget(&self) -> bool {
        self.counters.can_read(&self.budget)
            && Instant::now() < self.deadline
    }

    /// Resumen compacto del estado para enviar al modelo.
    pub fn operational_summary(&self) -> serde_json::Value {
        let pending: Vec<_> = self
            .questions
            .iter()
            .filter(|q| q.status == QuestionStatus::Pending)
            .map(|q| q.text.clone())
            .collect();
        let answered: Vec<_> = self
            .questions
            .iter()
            .filter(|q| q.status == QuestionStatus::Answered)
            .map(|q| {
                let refs: Vec<_> = q.evidence_ids.iter().map(|id| id.as_str().to_string()).collect();
                serde_json::json!({"question": q.text, "evidence": refs})
            })
            .collect();
        let doc_summary: Vec<_> = self
            .documents
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.source_id.as_str(),
                    "title": d.title,
                    "domain": d.domain,
                    "status": format!("{:?}", d.status).to_lowercase(),
                    "url": d.url,
                    "text": self.fragments.iter().filter(|f| f.source_id == d.source_id && !f.text.is_empty())
                        .map(|f| f.text.as_str()).chain(d.headers.iter().map(|f| f.text.as_str()))
                        .collect::<Vec<_>>().join("\n").chars().take(900).collect::<String>(),
                    "links": d.links.iter().take(12).map(|l| serde_json::json!({"id":l.link_id,"text":l.anchor_text,"url":l.url})).collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::json!({
            "resolved_question": self.resolved_question,
            "last_tool_feedback": self.last_tool_feedback,
            "intent": format!("{:?}", self.intent).to_lowercase(),
            "entities": self.entities.iter().map(|e| &e.name).collect::<Vec<_>>(),
            "pending_questions": pending,
            "answered_questions": answered,
            "documents": doc_summary,
            "operations_used": self.counters.operations,
            "budget_remaining": self.budget.total_operations.saturating_sub(self.counters.operations),
            "deadline_seconds_remaining": self.deadline.saturating_duration_since(Instant::now()).as_secs(),
        })
    }
}

// ── Contrato del modelo ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ModelDecision {
    Search {
        query: String,
        strategy: String,
        #[serde(default)]
        domains: Vec<String>,
        #[serde(default)]
        freshness: Option<String>,
    },
    Read {
        source_id: String,
        #[serde(default)]
        focus: Option<String>,
    },
    FollowLink {
        source_id: String,
        link_id: String,
        #[serde(default)]
        focus: Option<String>,
    },
    #[serde(rename = "github_read")]
    GitHubRead {
        source_id: String,
        resource: String,
        #[serde(default)]
        filters: Option<String>,
    },
    RedditRead {
        source_id: String,
        #[serde(default)]
        max_comments: Option<usize>,
        #[serde(default)]
        sort: Option<String>,
    },
    Finish {
        status: FinishStatus,
        #[serde(default)]
        pending_questions: Vec<String>,
    },
}

// ── Redacción y verificación ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Claim {
    pub text: String,
    pub evidence_ids: Vec<FragmentId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Draft {
    pub claims: Vec<Claim>,
    pub message: String,
    pub sources_used: Vec<SourceId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum VerificationStatus {
    Supported,
    PartiallySupported,
    Unsupported,
    Conflicting,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VerifiedClaim {
    pub claim: String,
    pub status: VerificationStatus,
    pub supporting_evidence: Vec<FragmentId>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VerificationResult {
    pub claims: Vec<VerifiedClaim>,
    pub can_finalize: bool,
}
