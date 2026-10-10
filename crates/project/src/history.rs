//! Unlimited undo and redo. Each step keeps a whole document version;
//! versions share every piece that did not change, so a step costs only the
//! pieces it touched.

use crate::document::Document;

#[derive(Clone, Debug)]
struct Entry {
    doc: Document,
    /// What produced this version, for "Undo move" style labels.
    label: String,
    /// Consecutive commands with the same key (a run of arrow-key nudges)
    /// become one step.
    coalesce: Option<String>,
}

#[derive(Clone, Debug)]
pub struct History {
    entries: Vec<Entry>,
    cursor: usize,
}

impl History {
    pub fn new(doc: Document) -> Self {
        History {
            entries: vec![Entry {
                doc,
                label: String::new(),
                coalesce: None,
            }],
            cursor: 0,
        }
    }

    pub fn current(&self) -> &Document {
        &self.entries[self.cursor].doc
    }

    /// Records a new version after the current one, dropping anything that
    /// was undone.
    pub fn push(&mut self, doc: Document, label: &str, coalesce: Option<String>) {
        self.entries.truncate(self.cursor + 1);
        let top = &mut self.entries[self.cursor];
        if self.cursor > 0 && coalesce.is_some() && top.coalesce == coalesce {
            top.doc = doc;
            return;
        }
        self.entries.push(Entry {
            doc,
            label: label.to_string(),
            coalesce,
        });
        self.cursor += 1;
    }

    pub fn undo(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.cursor + 1 >= self.entries.len() {
            return false;
        }
        self.cursor += 1;
        true
    }

    /// Label of the step undo would revert.
    pub fn undo_label(&self) -> Option<&str> {
        (self.cursor > 0).then(|| self.entries[self.cursor].label.as_str())
    }

    /// Label of the step redo would re-apply.
    pub fn redo_label(&self) -> Option<&str> {
        self.entries.get(self.cursor + 1).map(|e| e.label.as_str())
    }

    /// The version before the current one, if any.
    pub fn previous(&self) -> Option<&Document> {
        self.cursor.checked_sub(1).map(|i| &self.entries[i].doc)
    }

    /// The version after the current one, if any.
    pub fn next(&self) -> Option<&Document> {
        self.entries.get(self.cursor + 1).map(|e| &e.doc)
    }
}
