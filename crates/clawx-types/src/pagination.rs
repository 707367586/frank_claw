use serde::{Deserialize, Serialize};

/// Pagination parameters for list queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}

/// A paginated result set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl<T> PagedResult<T> {
    pub fn total_pages(&self) -> u64 {
        if self.per_page == 0 {
            return 0;
        }
        self.total.div_ceil(self.per_page)
    }
}
