//! Tab management for multiple open requests

use crate::models::{ApiRequest, ApiResponse};
use uuid::Uuid;

/// A tab representing an open request
#[derive(Debug, Clone)]
pub struct OpenRequestTab {
    pub id: String,
    pub request: ApiRequest,
    pub response: Option<ApiResponse>,
    pub is_modified: bool,
    pub collection_idx: Option<usize>,
    pub request_idx: Option<usize>,
}

impl OpenRequestTab {
    pub fn new(request: ApiRequest) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            request,
            response: None,
            is_modified: false,
            collection_idx: None,
            request_idx: None,
        }
    }

    pub fn from_collection(request: ApiRequest, col_idx: usize, req_idx: usize) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            request,
            response: None,
            is_modified: false,
            collection_idx: Some(col_idx),
            request_idx: Some(req_idx),
        }
    }

    pub fn title(&self) -> String {
        let prefix = if self.is_modified { "● " } else { "" };
        format!("{}{}", prefix, self.request.name)
    }
}

/// Tab manager
#[derive(Debug, Clone, Default)]
pub struct TabManager {
    pub tabs: Vec<OpenRequestTab>,
    pub active_tab_idx: Option<usize>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: vec![],
            active_tab_idx: None,
        }
    }

    /// Open a new tab with a request
    pub fn open_tab(
        &mut self,
        request: ApiRequest,
        col_idx: Option<usize>,
        req_idx: Option<usize>,
    ) {
        // Check if already open
        if let (Some(ci), Some(ri)) = (col_idx, req_idx) {
            if let Some(idx) = self
                .tabs
                .iter()
                .position(|t| t.collection_idx == Some(ci) && t.request_idx == Some(ri))
            {
                self.active_tab_idx = Some(idx);
                return;
            }
        }

        let tab = if let (Some(ci), Some(ri)) = (col_idx, req_idx) {
            OpenRequestTab::from_collection(request, ci, ri)
        } else {
            OpenRequestTab::new(request)
        };

        self.tabs.push(tab);
        self.active_tab_idx = Some(self.tabs.len() - 1);
    }

    /// Close a tab by index
    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.tabs.remove(idx);

            // Adjust active tab
            if self.tabs.is_empty() {
                self.active_tab_idx = None;
            } else if let Some(active) = self.active_tab_idx {
                if active > idx {
                    self.active_tab_idx = Some(active - 1);
                } else if active == idx {
                    self.active_tab_idx = Some(idx.min(self.tabs.len() - 1));
                }
            }
        }
    }

    /// Get the active tab
    pub fn active_tab(&self) -> Option<&OpenRequestTab> {
        self.active_tab_idx.and_then(|idx| self.tabs.get(idx))
    }

    /// Get the active tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut OpenRequestTab> {
        self.active_tab_idx.and_then(|idx| self.tabs.get_mut(idx))
    }

    /// Mark active tab as saved
    pub fn mark_saved(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            tab.is_modified = false;
        }
    }

    /// Check if there are any tabs
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_tabs() -> TabManager {
        let mut manager = TabManager::new();
        for name in ["first", "second", "third"] {
            manager.open_tab(ApiRequest::new(name), None, None);
        }
        manager
    }

    #[test]
    fn saved_request_reuses_its_existing_tab() {
        let mut manager = TabManager::new();
        manager.open_tab(ApiRequest::new("saved"), Some(0), Some(1));
        manager.open_tab(ApiRequest::new("unsaved"), None, None);
        manager.open_tab(ApiRequest::new("saved"), Some(0), Some(1));
        assert_eq!(manager.tabs.len(), 2);
        assert_eq!(manager.active_tab_idx, Some(0));
    }

    #[test]
    fn closing_earlier_tab_preserves_active_request() {
        let mut manager = three_tabs();
        manager.close_tab(0);
        assert_eq!(manager.active_tab_idx, Some(1));
        assert_eq!(manager.active_tab().unwrap().request.name, "third");
    }

    #[test]
    fn closing_active_tab_selects_neighbor() {
        let mut manager = three_tabs();
        manager.active_tab_idx = Some(1);
        manager.close_tab(1);
        assert_eq!(manager.active_tab().unwrap().request.name, "third");
        manager.close_tab(1);
        assert_eq!(manager.active_tab().unwrap().request.name, "first");
    }

    #[test]
    fn closing_last_tab_clears_selection_and_invalid_close_is_noop() {
        let mut manager = three_tabs();
        manager.close_tab(99);
        assert_eq!(manager.tabs.len(), 3);
        while manager.has_tabs() {
            manager.close_tab(0);
        }
        assert!(manager.active_tab().is_none());
        assert_eq!(manager.active_tab_idx, None);
    }

    #[test]
    fn tab_titles_track_saved_state() {
        let mut manager = three_tabs();
        manager.active_tab_mut().unwrap().is_modified = true;
        assert_eq!(manager.active_tab().unwrap().title(), "● third");
        manager.mark_saved();
        assert_eq!(manager.active_tab().unwrap().title(), "third");
    }

    #[test]
    fn unsaved_tabs_have_unique_identity() {
        let manager = three_tabs();
        assert_ne!(manager.tabs[0].id, manager.tabs[1].id);
    }

    #[test]
    fn closing_a_tab_without_an_active_selection_is_safe() {
        let mut manager = three_tabs();
        manager.active_tab_idx = None;
        manager.close_tab(0);
        manager.mark_saved();
        assert_eq!(manager.tabs.len(), 2);
        assert!(manager.active_tab_idx.is_none());
    }
}
