//! Text chunking for knowledge base indexing.

/// Default target chunk size in characters (roughly ~512 tokens).
const DEFAULT_CHUNK_SIZE: usize = 2048;
/// Overlap between chunks in characters.
const DEFAULT_OVERLAP: usize = 200;

/// A chunk of text with its index.
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub index: u32,
    pub content: String,
    pub token_count: u32,
}

/// Split text into overlapping chunks.
pub fn chunk_text(text: &str, chunk_size: Option<usize>, overlap: Option<usize>) -> Vec<TextChunk> {
    let size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let overlap = overlap.unwrap_or(DEFAULT_OVERLAP);

    if text.len() <= size {
        return vec![TextChunk {
            index: 0,
            content: text.to_string(),
            token_count: estimate_tokens(text),
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0u32;

    while start < text.len() {
        let end = (start + size).min(text.len());
        // Try to break at a paragraph or sentence boundary
        let actual_end = find_break_point(text, start, end);

        let content = text[start..actual_end].trim().to_string();
        if !content.is_empty() {
            chunks.push(TextChunk {
                index,
                content: content.clone(),
                token_count: estimate_tokens(&content),
            });
            index += 1;
        }

        if actual_end >= text.len() {
            break;
        }

        start = if actual_end > overlap {
            actual_end - overlap
        } else {
            actual_end
        };
    }

    chunks
}

/// Find a good break point near `end`, preferring paragraph > sentence > word boundaries.
fn find_break_point(text: &str, start: usize, end: usize) -> usize {
    if end >= text.len() {
        return text.len();
    }

    let segment = &text[start..end];

    // Try paragraph break (double newline)
    if let Some(pos) = segment.rfind("\n\n") {
        if pos > segment.len() / 2 {
            return start + pos + 2;
        }
    }

    // Try sentence break
    for delim in &[". ", ".\n", "! ", "? "] {
        if let Some(pos) = segment.rfind(delim) {
            if pos > segment.len() / 2 {
                return start + pos + delim.len();
            }
        }
    }

    // Try newline
    if let Some(pos) = segment.rfind('\n') {
        if pos > segment.len() / 2 {
            return start + pos + 1;
        }
    }

    // Try word break
    if let Some(pos) = segment.rfind(' ') {
        return start + pos + 1;
    }

    end
}

/// Rough token estimate (~4 chars per token for English).
fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 4.0).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_text_single_chunk() {
        let chunks = chunk_text("Hello, world!", None, None);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].content, "Hello, world!");
    }

    #[test]
    fn large_text_multiple_chunks() {
        let text = "a ".repeat(2000); // 4000 chars
        let chunks = chunk_text(&text, Some(1000), Some(100));
        assert!(chunks.len() >= 3, "expected 3+ chunks, got {}", chunks.len());
        // All chunks should have content
        for chunk in &chunks {
            assert!(!chunk.content.is_empty());
        }
    }

    #[test]
    fn chunks_are_indexed_sequentially() {
        let text = "word ".repeat(1000);
        let chunks = chunk_text(&text, Some(500), Some(50));
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i as u32);
        }
    }

    #[test]
    fn empty_text_returns_empty() {
        let chunks = chunk_text("", None, None);
        // Single empty-ish chunk or no chunks
        assert!(chunks.len() <= 1);
    }

    #[test]
    fn token_estimate_reasonable() {
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars / 4 ≈ 3
        assert_eq!(estimate_tokens(""), 0);
    }
}
