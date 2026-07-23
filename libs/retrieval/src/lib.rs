use async_trait::async_trait;
use embeddings::EmbeddingProvider;
use errors::ContextraError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use storage::vector_store::{SearchResult as VectorSearchResult, VectorStore};
use types::Metadata;
use uuid::Uuid;

const DEFAULT_RESULT_LIMIT: usize = 10;
const DEFAULT_CANDIDATE_MULTIPLIER: usize = 8;
const DEFAULT_CANDIDATE_FLOOR: usize = 64;
const KEYWORD_K1: f32 = 1.5;
const KEYWORD_B: f32 = 0.75;

const DEFAULT_TEXT_FIELDS: &[&str] = &[
    "content", "text", "title", "summary", "heading", "filename", "source",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryIntent {
    Search,
    Question,
    Summary,
    Comparison,
    Code,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueryEntityKind {
    Email,
    Hashtag,
    QuotedPhrase,
    FileType,
    Identifier,
    ProperNoun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryEntity {
    pub kind: QueryEntityKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzedQuery {
    pub original: String,
    pub normalized: String,
    pub intent: QueryIntent,
    pub entities: Vec<QueryEntity>,
    pub selected_collections: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionRule {
    collection: String,
    terms: Vec<String>,
}

impl CollectionRule {
    pub fn new(
        collection: impl Into<String>,
        terms: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            collection: collection.into(),
            terms: terms
                .into_iter()
                .map(|term| normalize_query_text(&term.into()))
                .filter(|term| !term.is_empty())
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryAnalyzer {
    default_collections: Vec<String>,
    collection_rules: Vec<CollectionRule>,
}

impl QueryAnalyzer {
    pub fn new(default_collections: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            default_collections: default_collections.into_iter().map(Into::into).collect(),
            collection_rules: Vec::new(),
        }
    }

    pub fn with_collection_rule(mut self, rule: CollectionRule) -> Self {
        self.collection_rules.push(rule);
        self
    }

    pub fn analyze(&self, query: &str) -> AnalyzedQuery {
        let normalized = normalize_query_text(query);
        let intent = detect_intent(query, &normalized);
        let entities = extract_entities(query);
        let selected_collections = self.select_collections(&normalized, &entities);

        AnalyzedQuery {
            original: query.to_string(),
            normalized,
            intent,
            entities,
            selected_collections,
        }
    }

    fn select_collections(&self, normalized: &str, entities: &[QueryEntity]) -> Vec<String> {
        let mut selected = Vec::new();
        let entity_terms: HashSet<&str> = entities
            .iter()
            .map(|entity| entity.value.as_str())
            .collect();

        for rule in &self.collection_rules {
            let matches_rule = rule
                .terms
                .iter()
                .any(|term| normalized.contains(term) || entity_terms.contains(term.as_str()));

            if matches_rule && !selected.contains(&rule.collection) {
                selected.push(rule.collection.clone());
            }
        }

        if selected.is_empty() {
            self.default_collections.clone()
        } else {
            selected
        }
    }
}

impl Default for QueryAnalyzer {
    fn default() -> Self {
        Self::new(["default"])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandedQuery {
    pub original: String,
    pub terms: Vec<String>,
}

impl ExpandedQuery {
    pub fn as_search_text(&self) -> String {
        self.terms.join(" ")
    }
}

pub trait TermExpansionProvider: Send + Sync {
    fn expand_term(&self, term: &str) -> Vec<String>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticTermExpansionProvider {
    terms: HashMap<String, Vec<String>>,
}

impl StaticTermExpansionProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_terms(
        mut self,
        term: impl Into<String>,
        expansions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.terms.insert(
            normalize_query_text(&term.into()),
            expansions
                .into_iter()
                .map(|expansion| normalize_query_text(&expansion.into()))
                .filter(|expansion| !expansion.is_empty())
                .collect(),
        );
        self
    }
}

impl TermExpansionProvider for StaticTermExpansionProvider {
    fn expand_term(&self, term: &str) -> Vec<String> {
        self.terms
            .get(term)
            .cloned()
            .unwrap_or_else(|| default_expansions(term))
    }
}

#[derive(Debug, Clone)]
pub struct QueryExpander<P = StaticTermExpansionProvider> {
    provider: P,
    max_terms: usize,
}

impl QueryExpander<StaticTermExpansionProvider> {
    pub fn new() -> Self {
        Self {
            provider: StaticTermExpansionProvider::new(),
            max_terms: 32,
        }
    }

    pub fn with_static_terms(
        mut self,
        term: impl Into<String>,
        expansions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.provider = self.provider.with_terms(term, expansions);
        self
    }
}

impl Default for QueryExpander<StaticTermExpansionProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> QueryExpander<P>
where
    P: TermExpansionProvider,
{
    pub fn with_provider(provider: P) -> Self {
        Self {
            provider,
            max_terms: 32,
        }
    }

    pub fn with_max_terms(mut self, max_terms: usize) -> Self {
        self.max_terms = max_terms.max(1);
        self
    }

    pub fn expand(&self, analyzed: &AnalyzedQuery) -> ExpandedQuery {
        let mut seen = HashSet::new();
        let mut terms = Vec::new();

        for token in tokenize(&analyzed.normalized) {
            self.push_term(&mut terms, &mut seen, &token);
            if terms.len() >= self.max_terms {
                break;
            }

            for expansion in self.provider.expand_term(&token) {
                self.push_term(&mut terms, &mut seen, &expansion);
                if terms.len() >= self.max_terms {
                    break;
                }
            }
        }

        for entity in &analyzed.entities {
            if terms.len() >= self.max_terms {
                break;
            }
            self.push_term(&mut terms, &mut seen, &normalize_query_text(&entity.value));
        }

        ExpandedQuery {
            original: analyzed.original.clone(),
            terms,
        }
    }

    fn push_term(&self, terms: &mut Vec<String>, seen: &mut HashSet<String>, term: &str) {
        let normalized = normalize_query_text(term);
        if !normalized.is_empty() && seen.insert(normalized.clone()) {
            terms.push(normalized);
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrievalFilter {
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub metadata: Vec<MetadataCondition>,
}

impl RetrievalFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn matches(&self, payload: &Metadata) -> bool {
        self.matches_scalar(payload, "collection", self.collection.as_deref())
            && self.matches_scalar(payload, "user_id", self.user_id.as_deref())
            && self.matches_scalar(payload, "organization_id", self.organization_id.as_deref())
            && self.matches_scalar(payload, "language", self.language.as_deref())
            && self.matches_scalar(payload, "file_type", self.file_type.as_deref())
            && self.matches_all_values(payload, "tags", &self.tags)
            && self.matches_any_value(payload, "permissions", &self.permissions)
            && self
                .metadata
                .iter()
                .all(|condition| condition.matches(payload))
    }

    fn matches_scalar(&self, payload: &Metadata, key: &str, expected: Option<&str>) -> bool {
        match expected {
            Some(expected) => metadata_value_matches(payload.get(key), expected),
            None => true,
        }
    }

    fn matches_all_values(&self, payload: &Metadata, key: &str, expected: &[String]) -> bool {
        expected
            .iter()
            .all(|value| metadata_value_contains(payload.get(key), value))
    }

    fn matches_any_value(&self, payload: &Metadata, key: &str, expected: &[String]) -> bool {
        expected.is_empty()
            || expected
                .iter()
                .any(|value| metadata_value_contains(payload.get(key), value))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataCondition {
    pub field: String,
    pub operator: FilterOperator,
    pub value: Value,
}

impl MetadataCondition {
    pub fn equals(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            field: field.into(),
            operator: FilterOperator::Equals,
            value: value.into(),
        }
    }

    pub fn contains(field: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            field: field.into(),
            operator: FilterOperator::Contains,
            value: value.into(),
        }
    }

    pub fn matches(&self, payload: &Metadata) -> bool {
        let Some(actual) = payload.get(&self.field) else {
            return false;
        };

        match self.operator {
            FilterOperator::Equals => values_equal(actual, &self.value),
            FilterOperator::NotEquals => !values_equal(actual, &self.value),
            FilterOperator::Contains => match self.value.as_str() {
                Some(expected) => metadata_value_contains(Some(actual), expected),
                None => false,
            },
            FilterOperator::GreaterThan => compare_numbers(actual, &self.value)
                .is_some_and(|ordering| ordering == Ordering::Greater),
            FilterOperator::GreaterThanOrEqual => compare_numbers(actual, &self.value)
                .is_some_and(|ordering| matches!(ordering, Ordering::Greater | Ordering::Equal)),
            FilterOperator::LessThan => compare_numbers(actual, &self.value)
                .is_some_and(|ordering| ordering == Ordering::Less),
            FilterOperator::LessThanOrEqual => compare_numbers(actual, &self.value)
                .is_some_and(|ordering| matches!(ordering, Ordering::Less | Ordering::Equal)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOperator {
    Equals,
    NotEquals,
    Contains,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalMode {
    #[default]
    Semantic,
    Keyword,
    Hybrid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalRequest {
    pub query: String,
    pub collection: String,
    #[serde(default)]
    pub mode: RetrievalMode,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filter: RetrievalFilter,
}

impl RetrievalRequest {
    pub fn semantic(query: impl Into<String>, collection: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            collection: collection.into(),
            mode: RetrievalMode::Semantic,
            limit,
            filter: RetrievalFilter::default(),
        }
    }

    pub fn keyword(query: impl Into<String>, collection: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            collection: collection.into(),
            mode: RetrievalMode::Keyword,
            limit,
            filter: RetrievalFilter::default(),
        }
    }

    pub fn hybrid(query: impl Into<String>, collection: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            collection: collection.into(),
            mode: RetrievalMode::Hybrid,
            limit,
            filter: RetrievalFilter::default(),
        }
    }
}

fn default_limit() -> usize {
    DEFAULT_RESULT_LIMIT
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievedDocument {
    pub id: Uuid,
    pub score: f32,
    pub semantic_score: Option<f32>,
    pub keyword_score: Option<f32>,
    #[serde(default)]
    pub payload: Metadata,
}

#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(
        &self,
        request: RetrievalRequest,
    ) -> Result<Vec<RetrievedDocument>, ContextraError>;
}

#[derive(Debug, Clone)]
pub struct VectorRetriever<S, E> {
    store: S,
    embeddings: E,
    text_fields: Vec<String>,
    candidate_multiplier: usize,
    candidate_floor: usize,
    hybrid_semantic_weight: f32,
    hybrid_keyword_weight: f32,
}

impl<S, E> VectorRetriever<S, E> {
    pub fn new(store: S, embeddings: E) -> Self {
        Self {
            store,
            embeddings,
            text_fields: DEFAULT_TEXT_FIELDS
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
            candidate_multiplier: DEFAULT_CANDIDATE_MULTIPLIER,
            candidate_floor: DEFAULT_CANDIDATE_FLOOR,
            hybrid_semantic_weight: 0.6,
            hybrid_keyword_weight: 0.4,
        }
    }

    pub fn with_text_fields(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.text_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_candidate_pool(mut self, multiplier: usize, floor: usize) -> Self {
        self.candidate_multiplier = multiplier.max(1);
        self.candidate_floor = floor.max(1);
        self
    }

    pub fn with_hybrid_weights(mut self, semantic: f32, keyword: f32) -> Self {
        self.hybrid_semantic_weight = semantic.max(0.0);
        self.hybrid_keyword_weight = keyword.max(0.0);
        self
    }

    fn candidate_limit(&self, request_limit: usize) -> usize {
        request_limit
            .max(1)
            .saturating_mul(self.candidate_multiplier)
            .max(self.candidate_floor)
    }
}

#[async_trait]
impl<S, E> Retriever for VectorRetriever<S, E>
where
    S: VectorStore + Send + Sync,
    E: EmbeddingProvider + Send + Sync,
{
    async fn retrieve(
        &self,
        request: RetrievalRequest,
    ) -> Result<Vec<RetrievedDocument>, ContextraError> {
        if request.query.trim().is_empty() {
            return Err(ContextraError::Validation(
                "retrieval query cannot be empty".to_string(),
            ));
        }
        if request.collection.trim().is_empty() {
            return Err(ContextraError::Validation(
                "retrieval collection cannot be empty".to_string(),
            ));
        }

        let limit = request.limit.max(1);
        let query_embedding = self.embed_query(&request.query).await?;
        let vector_limit = match request.mode {
            RetrievalMode::Semantic => self.candidate_limit(limit),
            RetrievalMode::Keyword | RetrievalMode::Hybrid => self.candidate_limit(limit),
        };
        let candidates = self
            .store
            .search(&request.collection, &query_embedding, vector_limit)
            .await?;
        let filtered = candidates
            .into_iter()
            .filter(|candidate| request.filter.matches(&candidate.payload))
            .collect::<Vec<_>>();

        let mut results = match request.mode {
            RetrievalMode::Semantic => semantic_results(filtered),
            RetrievalMode::Keyword => keyword_results(&request.query, filtered, &self.text_fields),
            RetrievalMode::Hybrid => hybrid_results(
                &request.query,
                filtered,
                &self.text_fields,
                self.hybrid_semantic_weight,
                self.hybrid_keyword_weight,
            ),
        };

        sort_retrieved(&mut results);
        results.truncate(limit);
        Ok(results)
    }
}

impl<S, E> VectorRetriever<S, E>
where
    E: EmbeddingProvider + Send + Sync,
{
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>, ContextraError> {
        let embeddings = self
            .embeddings
            .embed_batch(&[query.to_string()])
            .await
            .map_err(ContextraError::from)?;

        embeddings.into_iter().next().ok_or_else(|| {
            ContextraError::ProviderError(
                "embedding provider returned no query embedding".to_string(),
            )
        })
    }
}

fn semantic_results(candidates: Vec<VectorSearchResult>) -> Vec<RetrievedDocument> {
    candidates
        .into_iter()
        .map(|candidate| RetrievedDocument {
            id: candidate.id,
            score: candidate.score,
            semantic_score: Some(candidate.score),
            keyword_score: None,
            payload: candidate.payload,
        })
        .collect()
}

fn keyword_results(
    query: &str,
    candidates: Vec<VectorSearchResult>,
    text_fields: &[String],
) -> Vec<RetrievedDocument> {
    let keyword_scores = bm25_scores(query, &candidates, text_fields);

    candidates
        .into_iter()
        .filter_map(|candidate| {
            let score = keyword_scores.get(&candidate.id).copied().unwrap_or(0.0);
            (score > 0.0).then_some(RetrievedDocument {
                id: candidate.id,
                score,
                semantic_score: None,
                keyword_score: Some(score),
                payload: candidate.payload,
            })
        })
        .collect()
}

fn hybrid_results(
    query: &str,
    candidates: Vec<VectorSearchResult>,
    text_fields: &[String],
    semantic_weight: f32,
    keyword_weight: f32,
) -> Vec<RetrievedDocument> {
    let keyword_scores = bm25_scores(query, &candidates, text_fields);
    let max_keyword_score = keyword_scores.values().copied().fold(0.0_f32, f32::max);
    let total_weight = (semantic_weight + keyword_weight).max(f32::EPSILON);

    candidates
        .into_iter()
        .map(|candidate| {
            let keyword_score = keyword_scores.get(&candidate.id).copied().unwrap_or(0.0);
            let normalized_keyword = if max_keyword_score > 0.0 {
                keyword_score / max_keyword_score
            } else {
                0.0
            };
            let semantic_score = normalize_similarity(candidate.score);
            let score = ((semantic_score * semantic_weight)
                + (normalized_keyword * keyword_weight))
                / total_weight;

            RetrievedDocument {
                id: candidate.id,
                score,
                semantic_score: Some(candidate.score),
                keyword_score: Some(keyword_score),
                payload: candidate.payload,
            }
        })
        .collect()
}

fn bm25_scores(
    query: &str,
    candidates: &[VectorSearchResult],
    text_fields: &[String],
) -> HashMap<Uuid, f32> {
    let query_tokens = tokenize(&normalize_query_text(query));
    if query_tokens.is_empty() || candidates.is_empty() {
        return HashMap::new();
    }

    let query_terms: HashSet<String> = query_tokens.iter().cloned().collect();
    let documents = candidates
        .iter()
        .map(|candidate| {
            let text = payload_text(&candidate.payload, text_fields);
            let tokens = tokenize(&normalize_query_text(&text));
            (candidate.id, tokens)
        })
        .collect::<Vec<_>>();

    let average_length = documents
        .iter()
        .map(|(_, tokens)| tokens.len() as f32)
        .sum::<f32>()
        / documents.len() as f32;
    let average_length = average_length.max(1.0);

    let mut document_frequency: HashMap<String, usize> = HashMap::new();
    for (_, tokens) in &documents {
        let unique_terms = tokens.iter().cloned().collect::<HashSet<_>>();
        for term in unique_terms.intersection(&query_terms) {
            *document_frequency.entry(term.clone()).or_default() += 1;
        }
    }

    let document_count = documents.len() as f32;
    documents
        .into_iter()
        .map(|(id, tokens)| {
            let document_length = tokens.len() as f32;
            let mut frequencies: HashMap<String, usize> = HashMap::new();
            for token in tokens {
                if query_terms.contains(&token) {
                    *frequencies.entry(token).or_default() += 1;
                }
            }

            let score = frequencies
                .into_iter()
                .map(|(term, term_frequency)| {
                    let df = *document_frequency.get(&term).unwrap_or(&0) as f32;
                    let idf = ((document_count - df + 0.5) / (df + 0.5) + 1.0).ln();
                    let tf = term_frequency as f32;
                    let denominator = tf
                        + (KEYWORD_K1
                            * (1.0 - KEYWORD_B + KEYWORD_B * (document_length / average_length)));
                    idf * ((tf * (KEYWORD_K1 + 1.0)) / denominator)
                })
                .sum::<f32>();

            (id, score)
        })
        .collect()
}

fn sort_retrieved(results: &mut [RetrievedDocument]) {
    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn payload_text(payload: &Metadata, fields: &[String]) -> String {
    fields
        .iter()
        .filter_map(|field| payload.get(field))
        .flat_map(value_text)
        .collect::<Vec<_>>()
        .join(" ")
}

fn value_text(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items.iter().flat_map(value_text).collect(),
        Value::Object(map) => map.values().flat_map(value_text).collect(),
        Value::Number(number) => vec![number.to_string()],
        Value::Bool(value) => vec![value.to_string()],
        Value::Null => Vec::new(),
    }
}

fn normalize_similarity(score: f32) -> f32 {
    ((score + 1.0) / 2.0).clamp(0.0, 1.0)
}

fn detect_intent(original: &str, normalized: &str) -> QueryIntent {
    let first_token = normalized.split_whitespace().next().unwrap_or_default();

    if normalized.contains("compare ")
        || normalized.contains(" vs ")
        || normalized.contains(" versus ")
    {
        QueryIntent::Comparison
    } else if ["summarize", "summary", "recap", "tldr"].contains(&first_token) {
        QueryIntent::Summary
    } else if [
        "implement",
        "create",
        "update",
        "delete",
        "fix",
        "write",
        "run",
    ]
    .contains(&first_token)
    {
        QueryIntent::Command
    } else if [
        "code",
        "function",
        "struct",
        "trait",
        "rust",
        "python",
        "typescript",
        "javascript",
    ]
    .iter()
    .any(|term| normalized.contains(term))
    {
        QueryIntent::Code
    } else if original.trim_end().ends_with('?')
        || ["what", "why", "how", "when", "where", "who", "which"].contains(&first_token)
    {
        QueryIntent::Question
    } else {
        QueryIntent::Search
    }
}

fn extract_entities(query: &str) -> Vec<QueryEntity> {
    let mut entities = Vec::new();
    let mut seen = HashSet::new();

    for phrase in quoted_phrases(query) {
        push_entity(
            &mut entities,
            &mut seen,
            QueryEntityKind::QuotedPhrase,
            phrase,
        );
    }

    for raw in query.split_whitespace() {
        let token = raw.trim_matches(|character: char| {
            character.is_ascii_punctuation()
                && character != '#'
                && character != '@'
                && character != '.'
        });
        if token.is_empty() {
            continue;
        }

        if token.contains('@') && token.contains('.') {
            push_entity(
                &mut entities,
                &mut seen,
                QueryEntityKind::Email,
                token.to_lowercase(),
            );
        } else if let Some(tag) = token.strip_prefix('#') {
            if !tag.is_empty() {
                push_entity(
                    &mut entities,
                    &mut seen,
                    QueryEntityKind::Hashtag,
                    tag.to_lowercase(),
                );
            }
        } else if token.starts_with('.')
            && token.len() > 1
            && token[1..].chars().all(|c| c.is_ascii_alphanumeric())
        {
            push_entity(
                &mut entities,
                &mut seen,
                QueryEntityKind::FileType,
                token[1..].to_lowercase(),
            );
        } else if looks_like_identifier(token) {
            push_entity(
                &mut entities,
                &mut seen,
                QueryEntityKind::Identifier,
                token.to_string(),
            );
        }
    }

    for phrase in proper_noun_phrases(query) {
        push_entity(
            &mut entities,
            &mut seen,
            QueryEntityKind::ProperNoun,
            phrase,
        );
    }

    entities
}

fn push_entity(
    entities: &mut Vec<QueryEntity>,
    seen: &mut HashSet<(QueryEntityKind, String)>,
    kind: QueryEntityKind,
    value: String,
) {
    let value = value.trim().to_string();
    if !value.is_empty() && seen.insert((kind.clone(), value.clone())) {
        entities.push(QueryEntity { kind, value });
    }
}

fn quoted_phrases(query: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for character in query.chars() {
        if character == '"' {
            if in_quote && !current.trim().is_empty() {
                phrases.push(current.trim().to_string());
            }
            current.clear();
            in_quote = !in_quote;
        } else if in_quote {
            current.push(character);
        }
    }

    phrases
}

fn proper_noun_phrases(query: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut current = Vec::new();

    for raw in query.split_whitespace() {
        let token = raw.trim_matches(|character: char| character.is_ascii_punctuation());
        if token
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        {
            current.push(token.to_string());
        } else if current.len() > 1 {
            phrases.push(current.join(" "));
            current.clear();
        } else {
            current.clear();
        }
    }

    if current.len() > 1 {
        phrases.push(current.join(" "));
    }

    phrases
}

fn looks_like_identifier(token: &str) -> bool {
    token
        .chars()
        .any(|character| character == '_' || character == '-' || character == '/')
        || token.chars().any(|character| character.is_ascii_digit())
            && token
                .chars()
                .any(|character| character.is_ascii_alphabetic())
}

fn normalize_query_text(query: &str) -> String {
    query
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric()
                || character == '_'
                || character == '#'
                || character == '@'
            {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokenize(normalized: &str) -> Vec<String> {
    normalized
        .split_whitespace()
        .filter(|token| token.len() > 1 && !is_stop_word(token))
        .map(str::to_string)
        .collect()
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "with"
    )
}

fn default_expansions(term: &str) -> Vec<String> {
    match term {
        "bug" => vec!["error", "issue", "defect"],
        "fix" => vec!["repair", "resolve", "patch"],
        "auth" => vec!["authentication", "authorization", "login"],
        "login" => vec!["signin", "authentication", "session"],
        "vector" => vec!["embedding", "semantic", "similarity"],
        "embedding" => vec!["vector", "semantic", "representation"],
        "rust" => vec!["cargo", "crate", "trait"],
        "summary" => vec!["summarize", "recap", "overview"],
        "document" => vec!["file", "record", "source"],
        _ => Vec::new(),
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn metadata_value_matches(actual: Option<&Value>, expected: &str) -> bool {
    actual.is_some_and(|value| match value {
        Value::String(actual) => actual.eq_ignore_ascii_case(expected),
        Value::Number(number) => number.to_string() == expected,
        Value::Bool(value) => value.to_string() == expected,
        Value::Array(items) => items
            .iter()
            .any(|item| metadata_value_matches(Some(item), expected)),
        Value::Object(_) | Value::Null => false,
    })
}

fn metadata_value_contains(actual: Option<&Value>, expected: &str) -> bool {
    actual.is_some_and(|value| match value {
        Value::String(actual) => actual.eq_ignore_ascii_case(expected),
        Value::Array(items) => items
            .iter()
            .any(|item| metadata_value_contains(Some(item), expected)),
        Value::Object(map) => map
            .values()
            .any(|item| metadata_value_contains(Some(item), expected)),
        Value::Number(number) => number.to_string() == expected,
        Value::Bool(value) => value.to_string() == expected,
        Value::Null => false,
    })
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
}

fn compare_numbers(left: &Value, right: &Value) -> Option<Ordering> {
    let left = left.as_f64()?;
    let right = right.as_f64()?;
    left.partial_cmp(&right)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use embeddings::{Embedding, EmbeddingError};
    use serde_json::json;
    use storage::vector_store::{InMemoryVectorStore, VectorRecord};

    #[test]
    fn query_analyzer_normalizes_detects_entities_and_selects_collection() {
        let analyzer = QueryAnalyzer::new(["general"])
            .with_collection_rule(CollectionRule::new("engineering", ["rust", "cargo", "bug"]));

        let analyzed = analyzer.analyze(r#"Fix Rust bug BUG-123 for "Login Flow" #auth"#);

        assert_eq!(
            analyzed.normalized,
            "fix rust bug bug 123 for login flow #auth"
        );
        assert_eq!(analyzed.intent, QueryIntent::Command);
        assert_eq!(analyzed.selected_collections, vec!["engineering"]);
        assert!(analyzed.entities.iter().any(|entity| {
            entity.kind == QueryEntityKind::QuotedPhrase && entity.value == "Login Flow"
        }));
        assert!(
            analyzed.entities.iter().any(|entity| {
                entity.kind == QueryEntityKind::Hashtag && entity.value == "auth"
            })
        );
        assert!(analyzed.entities.iter().any(|entity| {
            entity.kind == QueryEntityKind::Identifier && entity.value == "BUG-123"
        }));
    }

    #[test]
    fn query_expander_combines_default_and_pluggable_terms() {
        let analyzer = QueryAnalyzer::default();
        let analyzed = analyzer.analyze("auth vector");
        let expander = QueryExpander::new()
            .with_static_terms("auth", ["oauth", "identity"])
            .with_max_terms(8);

        let expanded = expander.expand(&analyzed);

        assert!(expanded.terms.contains(&"auth".to_string()));
        assert!(expanded.terms.contains(&"oauth".to_string()));
        assert!(expanded.terms.contains(&"identity".to_string()));
        assert!(expanded.terms.contains(&"embedding".to_string()));
    }

    #[test]
    fn retrieval_filter_matches_metadata() {
        let filter = RetrievalFilter {
            collection: Some("docs".to_string()),
            user_id: Some("user-1".to_string()),
            organization_id: None,
            language: Some("en".to_string()),
            file_type: Some("md".to_string()),
            tags: vec!["rust".to_string()],
            permissions: vec!["read".to_string()],
            metadata: vec![MetadataCondition {
                field: "version".to_string(),
                operator: FilterOperator::GreaterThanOrEqual,
                value: json!(2),
            }],
        };

        let payload = metadata([
            ("collection", json!("docs")),
            ("user_id", json!("user-1")),
            ("language", json!("en")),
            ("file_type", json!("md")),
            ("tags", json!(["rust", "retrieval"])),
            ("permissions", json!(["read", "write"])),
            ("version", json!(3)),
        ]);

        assert!(filter.matches(&payload));
    }

    #[tokio::test]
    async fn vector_retriever_semantic_search_uses_vector_store()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = seeded_store().await?;
        let retriever = VectorRetriever::new(store, MockEmbeddingProvider);

        let results = retriever
            .retrieve(RetrievalRequest::semantic("ownership", "docs", 2))
            .await?;

        assert_eq!(
            results[0].payload.get("title"),
            Some(&json!("Rust Ownership"))
        );
        assert!(results[0].semantic_score.is_some());
        assert!(results[0].keyword_score.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn vector_retriever_keyword_search_scores_payload_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = seeded_store().await?;
        let retriever = VectorRetriever::new(store, MockEmbeddingProvider);

        let results = retriever
            .retrieve(RetrievalRequest::keyword(
                "redis cache invalidation",
                "docs",
                2,
            ))
            .await?;

        assert_eq!(results[0].payload.get("title"), Some(&json!("Cache Guide")));
        assert!(results[0].keyword_score.unwrap() > 0.0);
        assert!(results[0].semantic_score.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn vector_retriever_applies_metadata_filter() -> Result<(), Box<dyn std::error::Error>> {
        let store = seeded_store().await?;
        let retriever = VectorRetriever::new(store, MockEmbeddingProvider);
        let mut request = RetrievalRequest::semantic("ownership", "docs", 5);
        request.filter.tags = vec!["cache".to_string()];

        let results = retriever.retrieve(request).await?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].payload.get("title"), Some(&json!("Cache Guide")));
        Ok(())
    }

    async fn seeded_store() -> Result<InMemoryVectorStore, ContextraError> {
        let store = InMemoryVectorStore::new();
        store.create_collection("docs", 3).await?;
        store
            .upsert_vectors(
                "docs",
                &[
                    VectorRecord {
                        id: Uuid::from_u128(1),
                        embedding: vec![1.0, 0.0, 0.0],
                        payload: metadata([
                            ("title", json!("Rust Ownership")),
                            (
                                "content",
                                json!("Rust ownership uses borrowing lifetimes and move semantics"),
                            ),
                            ("tags", json!(["rust", "language"])),
                        ]),
                    },
                    VectorRecord {
                        id: Uuid::from_u128(2),
                        embedding: vec![0.0, 1.0, 0.0],
                        payload: metadata([
                            ("title", json!("Cache Guide")),
                            (
                                "content",
                                json!(
                                    "Redis cache invalidation cache TTL and cache stampede control"
                                ),
                            ),
                            ("tags", json!(["cache", "operations"])),
                        ]),
                    },
                    VectorRecord {
                        id: Uuid::from_u128(3),
                        embedding: vec![0.0, 0.0, 1.0],
                        payload: metadata([
                            ("title", json!("Deployment Notes")),
                            (
                                "content",
                                json!("Rollout checklist for service deployments"),
                            ),
                            ("tags", json!(["deploy"])),
                        ]),
                    },
                ],
            )
            .await?;
        Ok(store)
    }

    fn metadata(items: impl IntoIterator<Item = (&'static str, Value)>) -> Metadata {
        items
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[derive(Debug, Clone, Copy)]
    struct MockEmbeddingProvider;

    #[async_trait]
    impl EmbeddingProvider for MockEmbeddingProvider {
        async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Embedding>, EmbeddingError> {
            Ok(inputs
                .iter()
                .map(|input| {
                    let normalized = normalize_query_text(input);
                    if normalized.contains("ownership") || normalized.contains("rust") {
                        vec![1.0, 0.0, 0.0]
                    } else if normalized.contains("cache") || normalized.contains("redis") {
                        vec![0.0, 1.0, 0.0]
                    } else {
                        vec![0.0, 0.0, 1.0]
                    }
                })
                .collect())
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }
}
