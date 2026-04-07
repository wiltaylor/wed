//! Client capabilities advertised to language servers during `initialize`.

use lsp_types::{
    ClientCapabilities, CodeActionClientCapabilities, CompletionClientCapabilities,
    CompletionItemCapability, DocumentSymbolClientCapabilities, GeneralClientCapabilities,
    HoverClientCapabilities, MarkupKind, PublishDiagnosticsClientCapabilities,
    RenameClientCapabilities, SemanticTokensClientCapabilities,
    SemanticTokensClientCapabilitiesRequests, SignatureHelpClientCapabilities,
    TextDocumentClientCapabilities, TextDocumentSyncClientCapabilities,
    WorkspaceClientCapabilities, WorkspaceSymbolClientCapabilities,
};

pub fn client_capabilities() -> ClientCapabilities {
    let text_document = TextDocumentClientCapabilities {
        synchronization: Some(TextDocumentSyncClientCapabilities {
            dynamic_registration: Some(false),
            will_save: Some(false),
            will_save_wait_until: Some(false),
            did_save: Some(true),
        }),
        completion: Some(CompletionClientCapabilities {
            dynamic_registration: Some(false),
            completion_item: Some(CompletionItemCapability {
                snippet_support: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        hover: Some(HoverClientCapabilities {
            dynamic_registration: Some(false),
            content_format: Some(vec![MarkupKind::Markdown, MarkupKind::PlainText]),
        }),
        signature_help: Some(SignatureHelpClientCapabilities {
            dynamic_registration: Some(false),
            ..Default::default()
        }),
        definition: Some(Default::default()),
        references: Some(Default::default()),
        document_symbol: Some(DocumentSymbolClientCapabilities {
            dynamic_registration: Some(false),
            ..Default::default()
        }),
        code_action: Some(CodeActionClientCapabilities {
            dynamic_registration: Some(false),
            ..Default::default()
        }),
        formatting: Some(Default::default()),
        rename: Some(RenameClientCapabilities {
            dynamic_registration: Some(false),
            prepare_support: Some(true),
            ..Default::default()
        }),
        publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
            related_information: Some(true),
            version_support: Some(true),
            ..Default::default()
        }),
        semantic_tokens: Some(SemanticTokensClientCapabilities {
            dynamic_registration: Some(false),
            requests: SemanticTokensClientCapabilitiesRequests {
                range: Some(false),
                full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
            },
            token_types: vec![],
            token_modifiers: vec![],
            formats: vec![lsp_types::TokenFormat::RELATIVE],
            overlapping_token_support: Some(false),
            multiline_token_support: Some(false),
            server_cancel_support: Some(false),
            augments_syntax_tokens: Some(false),
        }),
        ..Default::default()
    };

    let workspace = WorkspaceClientCapabilities {
        symbol: Some(WorkspaceSymbolClientCapabilities {
            dynamic_registration: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    ClientCapabilities {
        text_document: Some(text_document),
        workspace: Some(workspace),
        general: Some(GeneralClientCapabilities::default()),
        ..Default::default()
    }
}
