use embeddings::{EmbeddingProvider, embed_batched};
use errors::ContextraError;
use scraper::{Html, Selector};
use serde_json::json;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use storage::vector_store::{VectorRecord, VectorStore};
use types::{Chunk, CollectionId, Document, DocumentId, Metadata};
use uuid::Uuid;

const DEFAULT_EMBEDDING_BATCH_SIZE: usize = 96;
const DEFAULT_EMBEDDING_CONCURRENCY: usize = 4;
const DEFAULT_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_MAX_EXTRACTED_BYTES: usize = 50 * 1024 * 1024;
const DEFAULT_MAX_PDF_NESTING_DEPTH: usize = 128;
const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionSecurityPolicy {
    pub max_file_bytes: u64,
    pub max_extracted_bytes: usize,
    pub max_pdf_nesting_depth: usize,
    pub max_compression_ratio: u64,
    pub allow_symlinks: bool,
    pub allowed_root: Option<PathBuf>,
}

impl Default for IngestionSecurityPolicy {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_extracted_bytes: DEFAULT_MAX_EXTRACTED_BYTES,
            max_pdf_nesting_depth: DEFAULT_MAX_PDF_NESTING_DEPTH,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
            allow_symlinks: false,
            allowed_root: None,
        }
    }
}

/// Validates a filesystem path before reading it for ingestion.
///
/// Rejects symlinks (unless explicitly permitted), non-regular files, files
/// exceeding `max_file_bytes`, and paths that escape `allowed_root` via
/// traversal or symlinked parent directories.
pub fn validate_safe_path(
    path: &Path,
    policy: &IngestionSecurityPolicy,
) -> Result<PathBuf, ContextraError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ContextraError::Validation(format!(
            "cannot inspect ingestion path '{}': {error}",
            path.display()
        ))
    })?;

    if !policy.allow_symlinks && metadata.file_type().is_symlink() {
        return Err(ContextraError::Validation(format!(
            "symlinks are not permitted during document ingestion: '{}'",
            path.display()
        )));
    }

    if !metadata.is_file() {
        return Err(ContextraError::Validation(format!(
            "ingestion path must be a regular file: '{}'",
            path.display()
        )));
    }

    if metadata.len() > policy.max_file_bytes {
        return Err(ContextraError::Validation(format!(
            "file '{}' exceeds maximum allowed size ({} > {} bytes)",
            path.display(),
            metadata.len(),
            policy.max_file_bytes
        )));
    }

    let canonical = std::fs::canonicalize(path).map_err(|error| {
        ContextraError::Validation(format!(
            "cannot canonicalize ingestion path '{}': {error}",
            path.display()
        ))
    })?;

    if let Some(root) = &policy.allowed_root {
        let canonical_root = std::fs::canonicalize(root).map_err(|error| {
            ContextraError::Validation(format!(
                "cannot canonicalize allowed ingestion root '{}': {error}",
                root.display()
            ))
        })?;
        if !canonical.starts_with(&canonical_root) {
            return Err(ContextraError::Validation(format!(
                "path '{}' escapes allowed ingestion root '{}'",
                path.display(),
                root.display()
            )));
        }
    }

    Ok(canonical)
}

/// Validates an archive entry path against path traversal, absolute paths,
/// Windows drive prefixes, and NUL bytes.
pub fn validate_archive_entry_path(entry_name: &str) -> Result<PathBuf, ContextraError> {
    if entry_name.is_empty() || entry_name.contains('\0') {
        return Err(ContextraError::Validation(
            "archive entry name is empty or contains NUL byte".to_string(),
        ));
    }

    if entry_name.contains('\\') {
        return Err(ContextraError::Validation(format!(
            "archive entry '{entry_name}' contains disallowed backslash separator"
        )));
    }

    let candidate = Path::new(entry_name);
    let mut clean = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ContextraError::Validation(format!(
                    "archive entry '{entry_name}' contains unsafe path component"
                )));
            }
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(ContextraError::Validation(format!(
            "archive entry '{entry_name}' resolves to an empty path"
        )));
    }

    Ok(clean)
}

/// Validates compressed vs. uncompressed archive entry sizes against
/// decompression-bomb limits.
pub fn validate_decompression_bounds(
    compressed_bytes: u64,
    uncompressed_bytes: u64,
    policy: &IngestionSecurityPolicy,
) -> Result<(), ContextraError> {
    if uncompressed_bytes > policy.max_extracted_bytes as u64 {
        return Err(ContextraError::Validation(format!(
            "extracted size ({uncompressed_bytes} bytes) exceeds maximum allowed ({} bytes)",
            policy.max_extracted_bytes
        )));
    }

    if uncompressed_bytes > 0 {
        let ratio = uncompressed_bytes
            .checked_div(compressed_bytes)
            .unwrap_or(u64::MAX);
        if ratio > policy.max_compression_ratio {
            return Err(ContextraError::Validation(format!(
                "suspicious compression ratio ({ratio}:1) exceeds maximum ({}:1)",
                policy.max_compression_ratio
            )));
        }
    }

    Ok(())
}

fn read_bounded_bytes(
    path: &Path,
    policy: &IngestionSecurityPolicy,
) -> Result<Vec<u8>, ContextraError> {
    let safe_path = validate_safe_path(path, policy)?;
    let file = std::fs::File::open(&safe_path)?;
    let limit = policy.max_file_bytes.saturating_add(1);
    let mut buffer = Vec::new();
    file.take(limit).read_to_end(&mut buffer)?;

    if buffer.len() as u64 > policy.max_file_bytes {
        return Err(ContextraError::Validation(format!(
            "file '{}' exceeds maximum allowed size ({} bytes)",
            path.display(),
            policy.max_file_bytes
        )));
    }

    Ok(buffer)
}

fn read_bounded_utf8(
    path: &Path,
    policy: &IngestionSecurityPolicy,
) -> Result<String, ContextraError> {
    let bytes = read_bounded_bytes(path, policy)?;
    String::from_utf8(bytes).map_err(|error| {
        ContextraError::Validation(format!(
            "file '{}' is not valid UTF-8: {error}",
            path.display()
        ))
    })
}

fn validate_pdf_structure(
    bytes: &[u8],
    policy: &IngestionSecurityPolicy,
) -> Result<(), ContextraError> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(ContextraError::Validation(
            "invalid PDF header: expected %PDF- signature".to_string(),
        ));
    }

    // Bound object/array/dictionary nesting depth before handing off to pdf-extract.
    let mut depth: usize = 0;
    let mut in_literal_string = false;
    let mut escaped = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if in_literal_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b')' {
                in_literal_string = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'(' => {
                in_literal_string = true;
                i += 1;
            }
            b'<' if i + 1 < bytes.len() && bytes[i + 1] == b'<' => {
                depth += 1;
                if depth > policy.max_pdf_nesting_depth {
                    return Err(ContextraError::Validation(format!(
                        "PDF nesting depth exceeds maximum allowed ({})",
                        policy.max_pdf_nesting_depth
                    )));
                }
                i += 2;
            }
            b'>' if i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                depth = depth.saturating_sub(1);
                i += 2;
            }
            b'[' => {
                depth += 1;
                if depth > policy.max_pdf_nesting_depth {
                    return Err(ContextraError::Validation(format!(
                        "PDF nesting depth exceeds maximum allowed ({})",
                        policy.max_pdf_nesting_depth
                    )));
                }
                i += 1;
            }
            b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDocument {
    pub content: String,
    pub metadata: Metadata,
}

impl ParsedDocument {
    pub fn new(content: impl Into<String>, metadata: Metadata) -> Self {
        Self {
            content: content.into(),
            metadata,
        }
    }
}

pub trait Parser: Send + Sync {
    fn parse_path(&self, path: &Path) -> Result<ParsedDocument, ContextraError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PlainTextParser;

impl Parser for PlainTextParser {
    fn parse_path(&self, path: &Path) -> Result<ParsedDocument, ContextraError> {
        let policy = IngestionSecurityPolicy::default();
        let content = read_bounded_utf8(path, &policy)?;
        Ok(ParsedDocument::new(content, parser_metadata(path, "text")))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownParser;

impl Parser for MarkdownParser {
    fn parse_path(&self, path: &Path) -> Result<ParsedDocument, ContextraError> {
        let policy = IngestionSecurityPolicy::default();
        let content = read_bounded_utf8(path, &policy)?;
        Ok(ParsedDocument::new(
            content,
            parser_metadata(path, "markdown"),
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PdfParser;

impl Parser for PdfParser {
    fn parse_path(&self, path: &Path) -> Result<ParsedDocument, ContextraError> {
        let policy = IngestionSecurityPolicy::default();
        let bytes = read_bounded_bytes(path, &policy)?;
        validate_pdf_structure(&bytes, &policy)?;
        let document = lopdf::Document::load_mem(&bytes).map_err(|error| {
            ContextraError::ProviderError(format!(
                "failed to parse PDF '{}': {error}",
                path.display()
            ))
        })?;
        let pages: Vec<u32> = document.get_pages().keys().copied().collect();
        let content = document.extract_text(&pages).map_err(|error| {
            ContextraError::ProviderError(format!(
                "failed to extract text from PDF '{}': {error}",
                path.display()
            ))
        })?;
        if content.len() > policy.max_extracted_bytes {
            return Err(ContextraError::Validation(format!(
                "extracted PDF text from '{}' exceeds maximum allowed size ({} bytes)",
                path.display(),
                policy.max_extracted_bytes
            )));
        }
        Ok(ParsedDocument::new(content, parser_metadata(path, "pdf")))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlParser;

impl Parser for HtmlParser {
    fn parse_path(&self, path: &Path) -> Result<ParsedDocument, ContextraError> {
        let policy = IngestionSecurityPolicy::default();
        let html = read_bounded_utf8(path, &policy)?;
        let document = Html::parse_document(&html);
        let selector =
            Selector::parse("title, h1, h2, h3, h4, h5, h6, p, li, pre").map_err(|error| {
                ContextraError::Internal(format!("failed to build HTML selector: {error}"))
            })?;

        let mut blocks = Vec::new();
        for element in document.select(&selector) {
            let text = normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "));
            if text.is_empty() {
                continue;
            }

            let tag = element.value().name();
            let block = match tag {
                "h1" => format!("# {text}"),
                "h2" => format!("## {text}"),
                "h3" => format!("### {text}"),
                "h4" => format!("#### {text}"),
                "h5" => format!("##### {text}"),
                "h6" => format!("###### {text}"),
                "li" => format!("- {text}"),
                _ => text,
            };
            blocks.push(block);
        }

        let content = if blocks.is_empty() {
            normalize_whitespace(&document.root_element().text().collect::<Vec<_>>().join(" "))
        } else {
            blocks.join("\n\n")
        };

        Ok(ParsedDocument::new(content, parser_metadata(path, "html")))
    }
}

pub trait Chunker: Send + Sync {
    fn chunk(
        &self,
        document_id: DocumentId,
        parsed: &ParsedDocument,
    ) -> Result<Vec<Chunk>, ContextraError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSizeChunker {
    chunk_size: usize,
    overlap: usize,
}

impl FixedSizeChunker {
    pub fn new(chunk_size: usize, overlap: usize) -> Result<Self, ContextraError> {
        if chunk_size == 0 {
            return Err(ContextraError::Validation(
                "chunk_size must be greater than zero".to_string(),
            ));
        }
        if overlap >= chunk_size {
            return Err(ContextraError::Validation(
                "overlap must be smaller than chunk_size".to_string(),
            ));
        }

        Ok(Self {
            chunk_size,
            overlap,
        })
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn overlap(&self) -> usize {
        self.overlap
    }
}

impl Chunker for FixedSizeChunker {
    fn chunk(
        &self,
        document_id: DocumentId,
        parsed: &ParsedDocument,
    ) -> Result<Vec<Chunk>, ContextraError> {
        fixed_size_chunks(
            document_id,
            parsed,
            self.chunk_size,
            self.overlap,
            "fixed_size",
            HashMap::new(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureAwareChunker {
    max_section_chars: usize,
    overlap: usize,
}

impl StructureAwareChunker {
    pub fn new(max_section_chars: usize, overlap: usize) -> Result<Self, ContextraError> {
        if max_section_chars == 0 {
            return Err(ContextraError::Validation(
                "max_section_chars must be greater than zero".to_string(),
            ));
        }
        if overlap >= max_section_chars {
            return Err(ContextraError::Validation(
                "overlap must be smaller than max_section_chars".to_string(),
            ));
        }

        Ok(Self {
            max_section_chars,
            overlap,
        })
    }
}

impl Chunker for StructureAwareChunker {
    fn chunk(
        &self,
        document_id: DocumentId,
        parsed: &ParsedDocument,
    ) -> Result<Vec<Chunk>, ContextraError> {
        let sections = heading_sections(&parsed.content);
        let mut chunks = Vec::new();

        for section in sections {
            let section_char_count = section.content.chars().count();
            let mut extra = HashMap::new();
            extra.insert("chunker".to_string(), json!("structure_aware"));
            extra.insert("heading".to_string(), json!(section.heading));
            extra.insert("heading_level".to_string(), json!(section.heading_level));

            if section_char_count <= self.max_section_chars {
                chunks.push(build_chunk(
                    document_id,
                    &parsed.metadata,
                    section.content,
                    section.start_offset,
                    section.end_offset,
                    chunks.len(),
                    extra,
                ));
            } else {
                let sub_document = ParsedDocument::new(section.content, parsed.metadata.clone());
                for mut chunk in fixed_size_chunks(
                    document_id,
                    &sub_document,
                    self.max_section_chars,
                    self.overlap,
                    "structure_aware",
                    extra.clone(),
                )? {
                    let relative_start = metadata_usize(&chunk.metadata, "start_offset")?;
                    let relative_end = metadata_usize(&chunk.metadata, "end_offset")?;
                    chunk.metadata.insert(
                        "start_offset".to_string(),
                        json!(section.start_offset + relative_start),
                    );
                    chunk.metadata.insert(
                        "end_offset".to_string(),
                        json!(section.start_offset + relative_end),
                    );
                    chunk
                        .metadata
                        .insert("chunk_index".to_string(), json!(chunks.len()));
                    chunks.push(chunk);
                }
            }
        }

        Ok(chunks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionOptions {
    pub embedding_batch_size: usize,
    pub embedding_concurrency: usize,
    pub ensure_collection: bool,
}

impl Default for IngestionOptions {
    fn default() -> Self {
        Self {
            embedding_batch_size: DEFAULT_EMBEDDING_BATCH_SIZE,
            embedding_concurrency: DEFAULT_EMBEDDING_CONCURRENCY,
            ensure_collection: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionProgress {
    Started {
        path: PathBuf,
    },
    Parsed {
        bytes: usize,
    },
    Chunked {
        chunks: usize,
    },
    Embedded {
        embeddings: usize,
    },
    Stored {
        records: usize,
    },
    Completed {
        document_id: DocumentId,
        chunks: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IngestionResult {
    pub document: Document,
    pub chunks: Vec<Chunk>,
}

type ProgressCallback = Arc<dyn Fn(IngestionProgress) + Send + Sync>;

pub struct IngestionPipeline<P, C, E, S> {
    parser: P,
    chunker: C,
    embedding_provider: E,
    vector_store: S,
    collection_name: String,
    collection_id: CollectionId,
    options: IngestionOptions,
    progress: Option<ProgressCallback>,
}

impl<P, C, E, S> IngestionPipeline<P, C, E, S>
where
    P: Parser,
    C: Chunker,
    E: EmbeddingProvider,
    S: VectorStore,
{
    pub fn new(
        parser: P,
        chunker: C,
        embedding_provider: E,
        vector_store: S,
        collection_name: impl Into<String>,
        collection_id: CollectionId,
    ) -> Self {
        Self {
            parser,
            chunker,
            embedding_provider,
            vector_store,
            collection_name: collection_name.into(),
            collection_id,
            options: IngestionOptions::default(),
            progress: None,
        }
    }

    pub fn with_options(mut self, options: IngestionOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_progress<F>(mut self, progress: F) -> Self
    where
        F: Fn(IngestionProgress) + Send + Sync + 'static,
    {
        self.progress = Some(Arc::new(progress));
        self
    }

    pub async fn ingest_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<IngestionResult, ContextraError> {
        let path = path.as_ref();
        self.emit(IngestionProgress::Started {
            path: path.to_path_buf(),
        });

        let mut parsed = self.parser.parse_path(path)?;
        parsed
            .metadata
            .insert("source_path".to_string(), json!(path.display().to_string()));

        self.ingest_parsed(parsed).await
    }

    pub async fn ingest_parsed(
        &self,
        mut parsed: ParsedDocument,
    ) -> Result<IngestionResult, ContextraError> {
        if self.options.embedding_batch_size == 0 {
            return Err(ContextraError::Validation(
                "embedding_batch_size must be greater than zero".to_string(),
            ));
        }

        if self.options.ensure_collection {
            self.vector_store
                .create_collection(&self.collection_name, self.embedding_provider.dimensions())
                .await?;
        }

        self.emit(IngestionProgress::Parsed {
            bytes: parsed.content.len(),
        });

        let document_id = DocumentId::new();
        let chunks = self.chunker.chunk(document_id, &parsed)?;
        parsed
            .metadata
            .insert("chunks_count".to_string(), json!(chunks.len()));
        let document = Document {
            id: document_id,
            collection_id: self.collection_id,
            content: parsed.content.clone(),
            metadata: parsed.metadata.clone(),
        };

        self.emit(IngestionProgress::Chunked {
            chunks: chunks.len(),
        });
        if chunks.is_empty() {
            self.emit(IngestionProgress::Completed {
                document_id,
                chunks: 0,
            });
            return Ok(IngestionResult { document, chunks });
        }

        let inputs = chunks
            .iter()
            .map(|chunk| chunk.content.clone())
            .collect::<Vec<_>>();
        let embeddings = embed_batched(
            &self.embedding_provider,
            &inputs,
            self.options.embedding_batch_size,
            self.options.embedding_concurrency,
        )
        .await?;
        self.emit(IngestionProgress::Embedded {
            embeddings: embeddings.len(),
        });

        let records = chunks
            .iter()
            .zip(embeddings)
            .map(|(chunk, embedding)| VectorRecord {
                id: chunk.id,
                embedding,
                payload: vector_payload(chunk),
            })
            .collect::<Vec<_>>();

        self.vector_store
            .upsert_vectors(&self.collection_name, &records)
            .await?;
        self.emit(IngestionProgress::Stored {
            records: records.len(),
        });
        self.emit(IngestionProgress::Completed {
            document_id,
            chunks: chunks.len(),
        });

        Ok(IngestionResult { document, chunks })
    }

    fn emit(&self, event: IngestionProgress) {
        if let Some(progress) = &self.progress {
            progress(event);
        }
    }
}

fn parser_metadata(path: &Path, parser: &str) -> Metadata {
    let mut metadata = Metadata::new();
    metadata.insert("parser".to_string(), json!(parser));
    if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
        metadata.insert("extension".to_string(), json!(extension));
    }
    metadata
}

fn fixed_size_chunks(
    document_id: DocumentId,
    parsed: &ParsedDocument,
    chunk_size: usize,
    overlap: usize,
    strategy: &str,
    extra_metadata: Metadata,
) -> Result<Vec<Chunk>, ContextraError> {
    if chunk_size == 0 {
        return Err(ContextraError::Validation(
            "chunk_size must be greater than zero".to_string(),
        ));
    }
    if overlap >= chunk_size {
        return Err(ContextraError::Validation(
            "overlap must be smaller than chunk_size".to_string(),
        ));
    }

    let boundaries = char_boundaries(&parsed.content);
    if boundaries.len() <= 1 {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut start_char = 0;
    let total_chars = boundaries.len() - 1;

    while start_char < total_chars {
        let end_char = (start_char + chunk_size).min(total_chars);
        let start_offset = boundaries[start_char];
        let end_offset = boundaries[end_char];
        let content = parsed.content[start_offset..end_offset].to_string();

        if !content.trim().is_empty() {
            let mut metadata = extra_metadata.clone();
            metadata
                .entry("chunker".to_string())
                .or_insert_with(|| json!(strategy));
            chunks.push(build_chunk(
                document_id,
                &parsed.metadata,
                content,
                start_offset,
                end_offset,
                chunks.len(),
                metadata,
            ));
        }

        if end_char == total_chars {
            break;
        }
        start_char = end_char - overlap;
    }

    Ok(chunks)
}

fn build_chunk(
    document_id: DocumentId,
    base_metadata: &Metadata,
    content: String,
    start_offset: usize,
    end_offset: usize,
    chunk_index: usize,
    extra_metadata: Metadata,
) -> Chunk {
    let mut metadata = base_metadata.clone();
    metadata.extend(extra_metadata);
    metadata.insert("start_offset".to_string(), json!(start_offset));
    metadata.insert("end_offset".to_string(), json!(end_offset));
    metadata.insert("chunk_index".to_string(), json!(chunk_index));

    Chunk {
        id: Uuid::now_v7(),
        document_id,
        content,
        metadata,
    }
}

fn char_boundaries(content: &str) -> Vec<usize> {
    content
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(content.len()))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeadingSection {
    heading: Option<String>,
    heading_level: Option<usize>,
    content: String,
    start_offset: usize,
    end_offset: usize,
}

fn heading_sections(content: &str) -> Vec<HeadingSection> {
    let mut line_starts = content
        .split_inclusive('\n')
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .collect::<Vec<_>>();

    if line_starts.is_empty() && !content.is_empty() {
        line_starts.push((0, content));
    }

    let mut sections = Vec::new();
    let mut current_start = 0;
    let mut current_heading = None;
    let mut current_heading_level = None;

    for (line_start, line) in line_starts {
        if let Some((level, heading)) = parse_heading(line) {
            if line_start > current_start {
                let section_content = content[current_start..line_start].trim().to_string();
                if !section_content.is_empty() {
                    sections.push(HeadingSection {
                        heading: current_heading.clone(),
                        heading_level: current_heading_level,
                        content: content[current_start..line_start].to_string(),
                        start_offset: current_start,
                        end_offset: line_start,
                    });
                }
            }
            current_start = line_start;
            current_heading = Some(heading);
            current_heading_level = Some(level);
        }
    }

    if current_start < content.len() {
        let section_content = content[current_start..].trim().to_string();
        if !section_content.is_empty() {
            sections.push(HeadingSection {
                heading: current_heading,
                heading_level: current_heading_level,
                content: content[current_start..].to_string(),
                start_offset: current_start,
                end_offset: content.len(),
            });
        }
    }

    if sections.is_empty() && !content.trim().is_empty() {
        sections.push(HeadingSection {
            heading: None,
            heading_level: None,
            content: content.to_string(),
            start_offset: 0,
            end_offset: content.len(),
        });
    }

    sections
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0 || level > 6 {
        return None;
    }

    let rest = trimmed[level..].trim();
    if rest.is_empty() || !trimmed[level..].starts_with(char::is_whitespace) {
        return None;
    }

    Some((level, rest.trim_end_matches('#').trim().to_string()))
}

fn metadata_usize(metadata: &Metadata, key: &str) -> Result<usize, ContextraError> {
    metadata
        .get(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| ContextraError::Internal(format!("chunk metadata missing {key}")))
}

fn vector_payload(chunk: &Chunk) -> Metadata {
    let mut payload = chunk.metadata.clone();
    payload.insert("chunk_id".to_string(), json!(chunk.id));
    payload.insert(
        "document_id".to_string(),
        json!(chunk.document_id.to_string()),
    );
    payload.insert("content".to_string(), json!(chunk.content));
    payload
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use embeddings::{Embedding, EmbeddingError};
    use std::sync::Mutex;
    use storage::vector_store::InMemoryVectorStore;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn plain_text_parser_reads_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = PlainTextParser.parse_path(&fixture("sample.txt"))?;

        assert!(parsed.content.contains("plain text fixture"));
        assert_eq!(parsed.metadata.get("parser"), Some(&json!("text")));

        Ok(())
    }

    #[test]
    fn markdown_parser_preserves_headings() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = MarkdownParser.parse_path(&fixture("sample.md"))?;

        assert!(parsed.content.starts_with("# Overview"));
        assert!(parsed.content.contains("## Details"));
        assert_eq!(parsed.metadata.get("parser"), Some(&json!("markdown")));

        Ok(())
    }

    #[test]
    fn html_parser_extracts_basic_blocks() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = HtmlParser.parse_path(&fixture("sample.html"))?;

        assert!(parsed.content.contains("# HTML Fixture"));
        assert!(parsed.content.contains("First paragraph"));
        assert!(parsed.content.contains("- List item"));
        assert_eq!(parsed.metadata.get("parser"), Some(&json!("html")));

        Ok(())
    }

    #[test]
    fn pdf_parser_extracts_fixture_text() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = PdfParser.parse_path(&fixture("sample.pdf"))?;

        assert!(parsed.content.contains("PDF fixture text"));
        assert_eq!(parsed.metadata.get("parser"), Some(&json!("pdf")));

        Ok(())
    }

    #[test]
    fn fixed_size_chunker_returns_overlapping_offsets() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = ParsedDocument::new("abcdefghij", Metadata::new());
        let document_id = DocumentId::new();
        let chunker = FixedSizeChunker::new(4, 1)?;

        let chunks = chunker.chunk(document_id, &parsed)?;

        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.content.as_str())
                .collect::<Vec<_>>(),
            vec!["abcd", "defg", "ghij"]
        );
        assert_eq!(chunks[1].metadata.get("start_offset"), Some(&json!(3)));
        assert_eq!(chunks[1].metadata.get("end_offset"), Some(&json!(7)));

        Ok(())
    }

    #[test]
    fn structure_aware_chunker_groups_by_headings() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = MarkdownParser.parse_path(&fixture("sample.md"))?;
        let document_id = DocumentId::new();
        let chunker = StructureAwareChunker::new(200, 0)?;

        let chunks = chunker.chunk(document_id, &parsed)?;

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].metadata.get("heading"), Some(&json!("Overview")));
        assert_eq!(chunks[1].metadata.get("heading"), Some(&json!("Details")));
        assert!(chunks[0].content.starts_with("# Overview"));
        assert!(chunks[1].content.starts_with("## Details"));

        Ok(())
    }

    #[tokio::test]
    async fn ingestion_pipeline_parses_chunks_embeds_and_stores()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = InMemoryVectorStore::new();
        let provider = MockEmbeddingProvider;
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured_events = Arc::clone(&events);
        let pipeline = IngestionPipeline::new(
            PlainTextParser,
            FixedSizeChunker::new(32, 0)?,
            provider,
            store.clone(),
            "ingestion-test",
            CollectionId::new(),
        )
        .with_progress(move |event| {
            captured_events.lock().unwrap().push(event);
        });

        let result = pipeline.ingest_path(fixture("sample.txt")).await?;
        let query = vec![1.0, 0.0, 0.0];
        let stored = store.search("ingestion-test", &query, 10).await?;

        assert!(!result.chunks.is_empty());
        assert_eq!(stored.len(), result.chunks.len());
        assert!(stored[0].payload.contains_key("content"));
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, IngestionProgress::Completed { .. }))
        );

        Ok(())
    }

    #[derive(Debug, Clone, Copy)]
    struct MockEmbeddingProvider;

    #[async_trait]
    impl EmbeddingProvider for MockEmbeddingProvider {
        async fn embed_batch(&self, inputs: &[String]) -> Result<Vec<Embedding>, EmbeddingError> {
            Ok(inputs.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
        }

        fn dimensions(&self) -> usize {
            3
        }

        fn model_name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn archive_entry_path_rejects_traversal_and_absolute_paths() {
        assert!(validate_archive_entry_path("../etc/passwd").is_err());
        assert!(validate_archive_entry_path("docs/../../secret.txt").is_err());
        assert!(validate_archive_entry_path("/etc/passwd").is_err());
        assert!(validate_archive_entry_path("dir\\file.txt").is_err());
        assert!(validate_archive_entry_path("file\0.txt").is_err());
        assert!(validate_archive_entry_path("").is_err());

        let clean = validate_archive_entry_path("docs/./readme.md").unwrap();
        assert_eq!(clean, PathBuf::from("docs/readme.md"));
    }

    #[test]
    fn decompression_bounds_reject_bombs_and_oversized_entries() {
        let policy = IngestionSecurityPolicy::default();
        assert!(validate_decompression_bounds(10, 10_000, &policy).is_err());
        assert!(validate_decompression_bounds(0, 100, &policy).is_err());
        assert!(
            validate_decompression_bounds(
                1_000_000,
                (policy.max_extracted_bytes as u64) + 1,
                &policy
            )
            .is_err()
        );
        assert!(validate_decompression_bounds(100, 1_000, &policy).is_ok());
    }

    #[test]
    fn pdf_structure_rejects_invalid_header_and_deep_nesting() {
        let policy = IngestionSecurityPolicy {
            max_pdf_nesting_depth: 16,
            ..IngestionSecurityPolicy::default()
        };

        assert!(validate_pdf_structure(b"not a pdf", &policy).is_err());

        let mut deep_pdf = b"%PDF-1.7\n".to_vec();
        deep_pdf.extend(std::iter::repeat_n(b'[', 32));
        deep_pdf.extend(std::iter::repeat_n(b']', 32));
        assert!(validate_pdf_structure(&deep_pdf, &policy).is_err());

        let valid_pdf = b"%PDF-1.7\n<< /Title (Nested [ok]) >>".to_vec();
        assert!(validate_pdf_structure(&valid_pdf, &policy).is_ok());
    }

    #[test]
    fn safe_path_rejects_oversized_files_and_root_escape() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let allowed_root = dir.path().join("allowed");
        std::fs::create_dir_all(&allowed_root)?;
        let inside = allowed_root.join("doc.txt");
        std::fs::write(&inside, "hello world")?;

        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, "secret")?;

        let root_policy = IngestionSecurityPolicy {
            allowed_root: Some(allowed_root.clone()),
            ..IngestionSecurityPolicy::default()
        };
        assert!(validate_safe_path(&inside, &root_policy).is_ok());
        assert!(validate_safe_path(&outside, &root_policy).is_err());

        let size_policy = IngestionSecurityPolicy {
            max_file_bytes: 5,
            ..IngestionSecurityPolicy::default()
        };
        assert!(validate_safe_path(&inside, &size_policy).is_err());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn safe_path_rejects_symlinks() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "content")?;
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link)?;

        let result = PlainTextParser.parse_path(&link);
        assert!(result.is_err());

        Ok(())
    }
}
