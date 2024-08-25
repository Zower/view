use std::{
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Stdio},
    sync::{mpsc::Receiver, Arc, Mutex},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument, Initialized},
    request::{Completion, HoverRequest, Initialize, Request},
    CodeActionCapabilityResolveSupport, CompletionParams, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, HoverParams, InitializedParams, PartialResultParams, Position,
    PositionEncodingKind, TextDocumentContentChangeEvent, WorkspaceFolder,
};

#[derive(Debug, Clone)]
pub struct LspResult {
    data: LspResultData,
}

#[derive(Debug, Clone)]
pub enum LspResultData {
    Hover(<HoverRequest as Request>::Result),
    Completion(<Completion as Request>::Result),
    Initialized,
}

// LSP sends message
#[derive(Debug, Clone)]
pub enum LspResponse {
    Result(LspResult),
    Notification(LspNotification),
}

#[derive(Debug, Clone)]
pub enum LspNotification {
    WorkDoneProgress(lsp_types::ProgressParams),
}

// Requests to the LSP server
// TODO: file
#[derive(Debug)]
pub struct LspRequest {
    pub file: PathBuf,
    pub data: LspRequestData,
}

#[derive(Debug)]
pub enum LspRequestData {
    // Request a hover
    Hover { line: u32, character: u32 },
    Completion { line: u32, character: u32 },
    DidChange { edit: LspEdit },
}

#[derive(Debug, Clone, Copy)]
enum LspSendRequestKind {
    Hover,
    Completion,
    Initialize,
}

#[derive(Debug)]
enum CalculatedReadResult {
    Response {
        id: u32,
        result: LspResultData,
    },
    Request {
        _id: u32,
        params: jsonrpc::RequestParam,
    },
    Notification {
        params: jsonrpc::NotificationParam,
    },
    Unknown(serde_json::Value),
}

pub trait LspResponseTransmitter: Clone + Send + 'static {
    type Error: std::error::Error;

    fn send(&self, event: LspResponse) -> Result<(), Self::Error>;
}

pub struct Lsp {
    next_id: u32,
    sent_requests: Arc<Mutex<ahash::HashMap<u32, SentRequestData>>>,
    writer: BufWriter<ChildStdin>,
    child: Child,
}

#[derive(Debug, Clone, Copy)]
struct SentRequestData {
    kind: LspSendRequestKind,
}

impl Lsp {
    fn new() -> (Self, BufReader<ChildStdout>) {
        let mut command = std::process::Command::new("rust-analyzer");

        command.stdin(Stdio::piped()).stdout(Stdio::piped());

        #[cfg(target_os = "windows")]
        command.creation_flags(0x08000000);

        let mut child = command.spawn().expect("Failed to start child");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let writer = std::io::BufWriter::new(stdin);
        let reader = std::io::BufReader::new(stdout);

        let this = Self {
            next_id: 0,
            sent_requests: Arc::new(Mutex::new(Default::default())),
            child,
            writer,
        };

        (this, reader)
    }

    fn init(&mut self, reader: &mut BufReader<ChildStdout>, workspace: &Path, file: &Path) {
        let params = init_params(workspace);

        let initialize_request = jsonrpc::request::<Initialize>(
            self.next_id(SentRequestData {
                kind: LspSendRequestKind::Initialize,
            }),
            params,
        );

        self.write_immediate(&initialize_request);

        let initialize_result =
            jsonrpc::read(reader, &self.sent_requests, &mut vec![], &mut String::new());

        match initialize_result {
            CalculatedReadResult::Response { .. } => {}
            _ => panic!("Expected initialize result after Initialize notification"),
        }

        let initialized_notification = jsonrpc::notification::<Initialized>(InitializedParams {});
        self.write_immediate(&initialized_notification);

        let path = file.canonicalize().expect("Path to exist");

        let file = std::fs::read_to_string(&path).unwrap();
        let message = jsonrpc::notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: url::Url::from_file_path(&path).unwrap(),
                language_id: "rust".into(),
                version: 1,
                text: file,
            },
        });

        self.write_immediate(&message);
    }

    fn send(sender: &impl LspResponseTransmitter, event: LspResponse) {
        sender.send(event).expect("Sending LSP event to work");
    }

    pub fn run(
        receiver: Receiver<LspRequest>,
        sender: impl LspResponseTransmitter,
        workspace: PathBuf,
        file: PathBuf,
    ) {
        let (mut lsp, mut reader) = Self::new();

        std::thread::spawn(move || {
            lsp.init(&mut reader, &workspace, &file);

            let sent_requests = lsp.sent_requests.clone();

            // Spawn the receiver
            std::thread::spawn(move || {
                let mut reusuable_buffer_string = String::new();
                let mut reusuable_buffer_vec = vec![];

                loop {
                    match jsonrpc::read(
                        &mut reader,
                        &sent_requests,
                        &mut reusuable_buffer_vec,
                        &mut reusuable_buffer_string,
                    ) {
                        CalculatedReadResult::Response { id, result } => {
                            let data = sent_requests
                                .lock()
                                .unwrap()
                                .remove(&id)
                                .expect("Receiver to remove request ids");

                            Self::send(&sender, LspResponse::Result(LspResult { data: result }))
                        }
                        CalculatedReadResult::Request { params, .. } => {
                            dbg!("{params:?}");
                        }
                        CalculatedReadResult::Notification { params } => match params {
                            jsonrpc::NotificationParam::Progress(progress) => Self::send(
                                &sender,
                                LspResponse::Notification(LspNotification::WorkDoneProgress(
                                    progress,
                                )),
                            ),
                        },
                        CalculatedReadResult::Unknown(value) => {
                            dbg!("Unprocessed jsonrpc message");

                            dbg!("{:?}", value);
                        }
                    }
                }
            });

            Self::run_sender(&mut lsp, receiver);

            lsp.child.kill().unwrap();
        });
    }

    fn run_sender(&mut self, request_receiver: Receiver<LspRequest>) {
        while let Ok(event) = request_receiver.recv() {
            let LspRequest { file, data } = event;

            match data {
                LspRequestData::Hover { line, character } => {
                    let message = jsonrpc::request::<HoverRequest>(
                        self.next_id(SentRequestData {
                            kind: LspSendRequestKind::Hover,
                        }),
                        HoverParams {
                            text_document_position_params: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: url::Url::from_file_path(&file).unwrap(),
                                },
                                position: Position { line, character },
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                        },
                    );

                    self.write_immediate(&message);
                }
                LspRequestData::Completion { line, character } => {
                    let message = jsonrpc::request::<Completion>(
                        self.next_id(SentRequestData {
                            kind: LspSendRequestKind::Completion,
                        }),
                        CompletionParams {
                            text_document_position: lsp_types::TextDocumentPositionParams {
                                text_document: lsp_types::TextDocumentIdentifier {
                                    uri: url::Url::from_file_path(&file).unwrap(),
                                },
                                position: Position { line, character },
                            },
                            work_done_progress_params: lsp_types::WorkDoneProgressParams {
                                work_done_token: None,
                            },
                            partial_result_params: PartialResultParams {
                                partial_result_token: None,
                            },
                            context: None,
                        },
                    );

                    self.write_immediate(&message);
                }
                LspRequestData::DidChange { edit } => {
                    let message = jsonrpc::notification::<DidChangeTextDocument>(
                        DidChangeTextDocumentParams {
                            text_document: lsp_types::VersionedTextDocumentIdentifier {
                                // TODO
                                version: 0,
                                uri: url::Url::from_file_path(&file).unwrap(),
                            },
                            content_changes: vec![TextDocumentContentChangeEvent {
                                range: Some(edit.range),
                                text: edit.text,
                                range_length: None,
                            }],
                        },
                    );

                    self.write_immediate(&message)
                }
            }
        }
    }

    fn write_immediate(&mut self, message: &str) {
        self.writer.write_all(message[..].as_bytes()).unwrap();

        self.writer.flush().unwrap();
    }

    fn next_id(&mut self, data: SentRequestData) -> u32 {
        let id = self.next_id;

        self.sent_requests.lock().unwrap().insert(id, data);

        self.next_id += 1;

        id
    }
}

#[derive(Debug)]
pub struct LspEdit {
    pub range: lsp_types::Range,
    pub text: String,
}

mod jsonrpc {
    use std::{
        io::{BufRead, Read},
        process::ChildStdout,
        sync::Mutex,
    };

    use lsp_types::{
        notification::Notification,
        request::{Completion, HoverRequest, Request},
    };
    use serde::{de::DeserializeOwned, Deserialize, Serialize};

    use crate::lsp::LspResultData;

    use super::{CalculatedReadResult, LspSendRequestKind, SentRequestData};

    #[derive(Serialize)]
    pub struct RequestMessage<T: serde::Serialize> {
        jsonrpc: &'static str,
        id: u32,
        method: &'static str,
        params: T,
    }

    #[derive(Serialize)]
    pub struct NotificationMessage<T: serde::Serialize> {
        jsonrpc: &'static str,
        method: &'static str,
        params: T,
    }

    #[derive(Deserialize, Debug)]
    pub struct RequestFromServer {
        id: u32,
        #[serde(flatten)]
        params: RequestParam,
    }

    #[derive(Deserialize, Debug)]
    pub struct NotificationFromServer {
        #[serde(flatten)]
        params: NotificationParam,
    }

    #[derive(Deserialize, Debug)]
    #[serde(tag = "method", content = "params")]
    pub enum RequestParam {
        #[serde(rename = "window/workDoneProgress/create")]
        WorkDoneProgressCreate(lsp_types::WorkDoneProgressCreateParams),
    }

    #[derive(Deserialize, Debug)]
    #[serde(tag = "method", content = "params")]
    pub enum NotificationParam {
        #[serde(rename = "$/progress")]
        Progress(lsp_types::ProgressParams),
    }

    pub fn request<T: Request>(id: u32, params: T::Params) -> String {
        let request = RequestMessage {
            jsonrpc: "2.0",
            id,
            method: T::METHOD,
            params,
        };

        let str =
            serde_json::to_string(&request).expect("Request message to be serializable to json");

        let len = str.len();

        format!("Content-Length: {len}\r\n\r\n{str}")
    }

    pub fn notification<T: Notification>(params: T::Params) -> String {
        let notification = NotificationMessage {
            jsonrpc: "2.0",
            method: T::METHOD,
            params,
        };

        let str = serde_json::to_string(&notification)
            .expect("Request message to be serializable to json");

        let len = str.len();

        format!("Content-Length: {len}\r\n\r\n{str}")
    }

    pub(super) fn read(
        reader: &mut std::io::BufReader<ChildStdout>,
        request_ids: &Mutex<ahash::HashMap<u32, SentRequestData>>,
        buffer_vec: &mut Vec<u8>,
        buffer_string: &mut String,
    ) -> CalculatedReadResult {
        let mut content_length: Option<usize> = None;

        loop {
            buffer_string.truncate(0);

            if reader.read_line(buffer_string).unwrap() == 0 {
                panic!();
                // return Err(Error::StreamClosed);
            };

            if buffer_string == "\r\n" {
                break;
            }

            let header = buffer_string.trim();

            let parts = header.split_once(": ");

            match parts {
                Some(("Content-Length", value)) => {
                    content_length = Some(value.parse().unwrap());
                }
                Some((_, _)) => {}
                None => {
                    // Workaround: Some non-conformant language servers will output logging and other garbage
                    // into the same stream as JSON-RPC messages. This can also happen from shell scripts that spawn
                    // the server. Skip such lines and log a warning.

                    // warn!("Failed to parse header: {:.unwrap()}", header);
                }
            }
        }

        buffer_vec.resize(content_length.unwrap(), 0);

        reader
            .read_exact(&mut buffer_vec[0..content_length.unwrap()])
            .unwrap();

        #[derive(Deserialize)]
        struct ResponseKind {
            id: u32,
            method: Option<String>,
        }

        fn deser<T: DeserializeOwned>(content: &[u8]) -> crate::Result<T> {
            let r = serde_json::from_slice(content);

            r.map_err(|err| {
                miette::miette!(
                    "Received unexpected data while parsing lsp message: Error: {err:?} \nData: \n\n{:?}",
                    String::from_utf8(Vec::from(content)).expect("Valid utf8")
                )
            })
        }

        fn deser_request<T: Request>(content: &[u8]) -> T::Result {
            #[derive(Deserialize)]
            struct ResultMessage<A> {
                result: A,
            }

            deser::<ResultMessage<T::Result>>(content).unwrap().result
        }

        let id: Result<ResponseKind, _> = serde_json::from_slice(buffer_vec);

        match id {
            Ok(ResponseKind { id, method: None }) => {
                let data = { *request_ids.lock().unwrap().get(&id).unwrap() };

                CalculatedReadResult::Response {
                    id,
                    result: match data.kind {
                        LspSendRequestKind::Hover => {
                            LspResultData::Hover(deser_request::<HoverRequest>(buffer_vec))
                        }
                        LspSendRequestKind::Completion => {
                            LspResultData::Completion(deser_request::<Completion>(buffer_vec))
                        }
                        LspSendRequestKind::Initialize => LspResultData::Initialized,
                    },
                }
            }
            Ok(ResponseKind {
                id: _,
                method: Some(_),
            }) => deser::<RequestFromServer>(buffer_vec)
                .map(|req| CalculatedReadResult::Request {
                    _id: req.id,
                    params: req.params,
                })
                .unwrap_or_else(|_| CalculatedReadResult::Unknown(deser(buffer_vec).unwrap())),
            Err(_) => deser::<NotificationFromServer>(buffer_vec)
                .map(|not| CalculatedReadResult::Notification { params: not.params })
                .unwrap_or_else(|_| {
                    let content = &buffer_vec;
                    CalculatedReadResult::Unknown(deser(content).unwrap())
                }),
        }
    }
}

fn init_params(workspace: &Path) -> lsp_types::InitializeParams {
    lsp_types::InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: url::Url::parse(&format!("file://{}", workspace.to_string_lossy().as_ref()))
                .unwrap(),
            name: String::from("flare"),
        }]),
        // workspace_folders: Some(self.workspace_folders.lock().clone()),
        // root_path is obsolete, but some clients like pyright still use it so we specify both.
        // clients will prefer _uri if possible
        // root_path: self.root_path.to_str().map(|path| path.to_owned()),
        // root_uri: self.root_uri.clone(),
        // initialization_options: self.config.clone(),
        capabilities: lsp_types::ClientCapabilities {
            workspace: Some(lsp_types::WorkspaceClientCapabilities {
                configuration: Some(true),
                did_change_configuration: Some(lsp_types::DynamicRegistrationClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                workspace_folders: Some(true),
                apply_edit: Some(true),
                symbol: Some(lsp_types::WorkspaceSymbolClientCapabilities {
                    dynamic_registration: Some(false),
                    ..Default::default()
                }),
                execute_command: Some(lsp_types::DynamicRegistrationClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                inlay_hint: Some(lsp_types::InlayHintWorkspaceClientCapabilities {
                    refresh_support: Some(false),
                }),
                workspace_edit: Some(lsp_types::WorkspaceEditClientCapabilities {
                    document_changes: Some(true),
                    resource_operations: Some(vec![
                        lsp_types::ResourceOperationKind::Create,
                        lsp_types::ResourceOperationKind::Rename,
                        lsp_types::ResourceOperationKind::Delete,
                    ]),
                    failure_handling: Some(lsp_types::FailureHandlingKind::Abort),
                    normalizes_line_endings: Some(false),
                    change_annotation_support: None,
                }),
                did_change_watched_files: Some(
                    lsp_types::DidChangeWatchedFilesClientCapabilities {
                        dynamic_registration: Some(true),
                        relative_pattern_support: Some(false),
                    },
                ),
                file_operations: Some(lsp_types::WorkspaceFileOperationsClientCapabilities {
                    will_rename: Some(true),
                    did_rename: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                completion: Some(lsp_types::CompletionClientCapabilities {
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        snippet_support: Some(true),
                        resolve_support: Some(lsp_types::CompletionItemCapabilityResolveSupport {
                            properties: vec![
                                String::from("documentation"),
                                String::from("detail"),
                                String::from("additionalTextEdits"),
                            ],
                        }),
                        insert_replace_support: Some(true),
                        deprecated_support: Some(true),
                        tag_support: Some(lsp_types::TagSupport {
                            value_set: vec![lsp_types::CompletionItemTag::DEPRECATED],
                        }),
                        ..Default::default()
                    }),
                    completion_item_kind: Some(lsp_types::CompletionItemKindCapability {
                        ..Default::default()
                    }),
                    context_support: None, // additional context information Some(true)
                    ..Default::default()
                }),
                hover: Some(lsp_types::HoverClientCapabilities {
                    // if not specified, rust-analyzer returns plaintext marked as markdown but
                    // badly formatted.
                    content_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                    ..Default::default()
                }),
                signature_help: Some(lsp_types::SignatureHelpClientCapabilities {
                    signature_information: Some(lsp_types::SignatureInformationSettings {
                        documentation_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                        parameter_information: Some(lsp_types::ParameterInformationSettings {
                            label_offset_support: Some(true),
                        }),
                        active_parameter_support: Some(true),
                    }),
                    ..Default::default()
                }),
                rename: Some(lsp_types::RenameClientCapabilities {
                    dynamic_registration: Some(false),
                    prepare_support: Some(true),
                    prepare_support_default_behavior: None,
                    honors_change_annotations: Some(false),
                }),
                code_action: Some(lsp_types::CodeActionClientCapabilities {
                    code_action_literal_support: Some(lsp_types::CodeActionLiteralSupport {
                        code_action_kind: lsp_types::CodeActionKindLiteralSupport {
                            value_set: [
                                lsp_types::CodeActionKind::EMPTY,
                                lsp_types::CodeActionKind::QUICKFIX,
                                lsp_types::CodeActionKind::REFACTOR,
                                lsp_types::CodeActionKind::REFACTOR_EXTRACT,
                                lsp_types::CodeActionKind::REFACTOR_INLINE,
                                lsp_types::CodeActionKind::REFACTOR_REWRITE,
                                lsp_types::CodeActionKind::SOURCE,
                                lsp_types::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                            ]
                            .iter()
                            .map(|kind| kind.as_str().to_string())
                            .collect(),
                        },
                    }),
                    is_preferred_support: Some(true),
                    disabled_support: Some(true),
                    data_support: Some(true),
                    resolve_support: Some(CodeActionCapabilityResolveSupport {
                        properties: vec!["edit".to_owned(), "command".to_owned()],
                    }),
                    ..Default::default()
                }),
                publish_diagnostics: Some(lsp_types::PublishDiagnosticsClientCapabilities {
                    version_support: Some(true),
                    ..Default::default()
                }),
                inlay_hint: Some(lsp_types::InlayHintClientCapabilities {
                    dynamic_registration: Some(false),
                    resolve_support: None,
                }),
                ..Default::default()
            }),
            window: Some(lsp_types::WindowClientCapabilities {
                work_done_progress: Some(true),
                ..Default::default()
            }),
            general: Some(lsp_types::GeneralClientCapabilities {
                position_encodings: Some(vec![
                    PositionEncodingKind::UTF8,
                    PositionEncodingKind::UTF32,
                    PositionEncodingKind::UTF16,
                ]),
                ..Default::default()
            }),
            ..Default::default()
        },
        trace: None,
        client_info: Some(lsp_types::ClientInfo {
            name: String::from("flare"),
            version: Some(String::from("0.1.0")),
        }),
        locale: None, // TODO
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {}
