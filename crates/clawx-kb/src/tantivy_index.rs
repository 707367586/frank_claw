//! Tantivy BM25 full-text search index for knowledge chunks.

use std::path::Path;

use clawx_types::error::{ClawxError, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

/// A hit returned from the Tantivy BM25 search.
#[derive(Debug, Clone)]
pub struct TantivyHit {
    pub chunk_id: String,
    pub document_id: String,
    pub document_path: String,
    pub content: String,
    pub chunk_index: u32,
    pub bm25_score: f32,
}

/// Wraps a Tantivy index for BM25 full-text search over knowledge chunks.
pub struct TantivyIndex {
    index: Index,
    reader: IndexReader,
    writer: std::sync::Mutex<IndexWriter>,
    // Schema fields
    f_chunk_id: Field,
    f_document_id: Field,
    f_document_path: Field,
    f_content: Field,
    f_chunk_index: Field,
}

impl std::fmt::Debug for TantivyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TantivyIndex").finish()
    }
}

impl TantivyIndex {
    fn build_schema() -> (Schema, Field, Field, Field, Field, Field) {
        let mut builder = Schema::builder();
        let f_chunk_id = builder.add_text_field("chunk_id", STRING | STORED);
        let f_document_id = builder.add_text_field("document_id", STRING | STORED);
        let f_document_path = builder.add_text_field("document_path", STRING | STORED);
        let f_content = builder.add_text_field("content", TEXT | STORED);
        let f_chunk_index = builder.add_u64_field("chunk_index", STORED);
        let schema = builder.build();
        (schema, f_chunk_id, f_document_id, f_document_path, f_content, f_chunk_index)
    }

    /// Open or create a Tantivy index at the given directory path.
    pub fn open(path: &Path) -> Result<Self> {
        let (schema, f_chunk_id, f_document_id, f_document_path, f_content, f_chunk_index) =
            Self::build_schema();

        std::fs::create_dir_all(path)
            .map_err(|e| ClawxError::Internal(format!("failed to create index dir: {}", e)))?;

        let dir = tantivy::directory::MmapDirectory::open(path)
            .map_err(|e| ClawxError::Internal(format!("tantivy dir error: {}", e)))?;

        let index = Index::open_or_create(dir, schema)
            .map_err(|e| ClawxError::Internal(format!("tantivy index error: {}", e)))?;

        Self::from_index(index, f_chunk_id, f_document_id, f_document_path, f_content, f_chunk_index)
    }

    /// Open an in-RAM index for testing.
    pub fn open_in_ram() -> Result<Self> {
        let (schema, f_chunk_id, f_document_id, f_document_path, f_content, f_chunk_index) =
            Self::build_schema();

        let index = Index::create_in_ram(schema);

        Self::from_index(index, f_chunk_id, f_document_id, f_document_path, f_content, f_chunk_index)
    }

    fn from_index(
        index: Index,
        f_chunk_id: Field,
        f_document_id: Field,
        f_document_path: Field,
        f_content: Field,
        f_chunk_index: Field,
    ) -> Result<Self> {
        let writer = index
            .writer(50_000_000) // 50 MB heap
            .map_err(|e| ClawxError::Internal(format!("tantivy writer error: {}", e)))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| ClawxError::Internal(format!("tantivy reader error: {}", e)))?;

        Ok(Self {
            index,
            reader,
            writer: std::sync::Mutex::new(writer),
            f_chunk_id,
            f_document_id,
            f_document_path,
            f_content,
            f_chunk_index,
        })
    }

    /// Index a single chunk.
    pub fn add_chunk(
        &self,
        chunk_id: &str,
        document_id: &str,
        document_path: &str,
        content: &str,
        chunk_index: u32,
    ) -> Result<()> {
        let writer = self.writer.lock().map_err(|e| ClawxError::Internal(format!("lock error: {}", e)))?;
        writer
            .add_document(doc!(
                self.f_chunk_id => chunk_id,
                self.f_document_id => document_id,
                self.f_document_path => document_path,
                self.f_content => content,
                self.f_chunk_index => chunk_index as u64,
            ))
            .map_err(|e| ClawxError::Internal(format!("tantivy add error: {}", e)))?;
        Ok(())
    }

    /// Delete all chunks belonging to a document.
    pub fn delete_by_document(&self, document_id: &str) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|e| ClawxError::Internal(format!("lock error: {}", e)))?;
        let term = Term::from_field_text(self.f_document_id, document_id);
        writer.delete_term(term);
        // We don't commit here; caller should call commit() explicitly.
        let _ = &mut writer; // suppress unused warning
        Ok(())
    }

    /// Commit pending writes so they become visible to readers.
    pub fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|e| ClawxError::Internal(format!("lock error: {}", e)))?;
        writer
            .commit()
            .map_err(|e| ClawxError::Internal(format!("tantivy commit error: {}", e)))?;
        self.reader.reload().map_err(|e| ClawxError::Internal(format!("tantivy reload error: {}", e)))?;
        Ok(())
    }

    /// Search the index using BM25 scoring.
    pub fn search(&self, query_text: &str, top_n: usize) -> Result<Vec<TantivyHit>> {
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.f_content]);

        let query = query_parser
            .parse_query(query_text)
            .map_err(|e| ClawxError::Internal(format!("tantivy query parse error: {}", e)))?;

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(top_n))
            .map_err(|e| ClawxError::Internal(format!("tantivy search error: {}", e)))?;

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| ClawxError::Internal(format!("tantivy doc fetch error: {}", e)))?;

            let chunk_id = doc
                .get_first(self.f_chunk_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let document_id = doc
                .get_first(self.f_document_id)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let document_path = doc
                .get_first(self.f_document_path)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let content = doc
                .get_first(self.f_content)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let chunk_index = doc
                .get_first(self.f_chunk_index)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            hits.push(TantivyHit {
                chunk_id,
                document_id,
                document_path,
                content,
                chunk_index,
                bm25_score: score,
            });
        }

        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tantivy_index_and_search() {
        let idx = TantivyIndex::open_in_ram().unwrap();

        idx.add_chunk("c1", "d1", "/docs/rust.md", "Rust is a systems programming language", 0).unwrap();
        idx.add_chunk("c2", "d1", "/docs/rust.md", "Rust focuses on safety and performance", 1).unwrap();
        idx.add_chunk("c3", "d2", "/docs/python.md", "Python is a dynamic scripting language", 0).unwrap();
        idx.commit().unwrap();

        let hits = idx.search("Rust programming", 10).unwrap();
        assert!(!hits.is_empty(), "should find results for 'Rust programming'");
        // The top hit should be about Rust
        assert!(hits[0].content.contains("Rust"));
        assert!(hits[0].bm25_score > 0.0);
        assert_eq!(hits[0].document_id, "d1");
    }

    #[test]
    fn test_tantivy_delete_by_document() {
        let idx = TantivyIndex::open_in_ram().unwrap();

        idx.add_chunk("c1", "d1", "/docs/a.md", "Alpha bravo charlie", 0).unwrap();
        idx.add_chunk("c2", "d1", "/docs/a.md", "Delta echo foxtrot", 1).unwrap();
        idx.add_chunk("c3", "d2", "/docs/b.md", "Alpha golf hotel", 0).unwrap();
        idx.commit().unwrap();

        // Verify we can find doc d1 content
        let hits = idx.search("Alpha", 10).unwrap();
        assert_eq!(hits.len(), 2);

        // Delete doc d1
        idx.delete_by_document("d1").unwrap();
        idx.commit().unwrap();

        // Only d2 should remain
        let hits = idx.search("Alpha", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_id, "d2");
    }

    #[test]
    fn test_tantivy_empty_search() {
        let idx = TantivyIndex::open_in_ram().unwrap();
        idx.commit().unwrap();

        let hits = idx.search("anything", 10).unwrap();
        assert!(hits.is_empty());
    }
}
