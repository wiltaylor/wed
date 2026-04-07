//! Per-URI diagnostic store fed by `textDocument/publishDiagnostics`.

use std::collections::HashMap;

use lsp_types::{Diagnostic, PublishDiagnosticsParams, Uri};

#[derive(Default, Debug)]
pub struct DiagnosticStore {
    pub per_uri: HashMap<Uri, Vec<Diagnostic>>,
}

impl DiagnosticStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&mut self, params: PublishDiagnosticsParams) {
        self.per_uri.insert(params.uri, params.diagnostics);
    }

    pub fn get(&self, uri: &Uri) -> &[Diagnostic] {
        self.per_uri
            .get(uri)
            .map(|v: &Vec<Diagnostic>| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn clear(&mut self, uri: &Uri) {
        self.per_uri.remove(uri);
    }

    pub fn total(&self) -> usize {
        self.per_uri
            .values()
            .map(|v: &Vec<Diagnostic>| v.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{DiagnosticSeverity, Position, Range};

    fn diag(msg: &str) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity: Some(DiagnosticSeverity::ERROR),
            message: msg.into(),
            ..Default::default()
        }
    }

    #[test]
    fn publish_and_get() {
        let mut s = DiagnosticStore::new();
        let uri: Uri = "file:///x.rs".parse().unwrap();
        s.publish(PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: vec![diag("oops"), diag("bad")],
            version: None,
        });
        assert_eq!(s.get(&uri).len(), 2);
        assert_eq!(s.total(), 2);
        s.clear(&uri);
        assert_eq!(s.get(&uri).len(), 0);
    }
}
