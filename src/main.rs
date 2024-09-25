use std::{io, path::PathBuf};

use components::root::Root;

use cosmic_text::FontSystem;
use miette::IntoDiagnostic;
use paladin_view::{prelude::*, CustomWidget};
use paladinc::{lsp::LspResponseTransmitter, ts::highlight};
mod components;

fn main() -> paladin_view::Result<()> {
    run(Root)
}

pub struct BufferElement {
    path: String,
}

struct BufferWidget {
    buffer: paladinc::Buffer,
    text: paladin_view::Text,
    qc: tree_sitter::QueryCursor,
    query: tree_sitter::Query,
}

impl BufferElement {
    pub fn new(path: impl Into<String>) -> impl Element {
        Self { path: path.into() }
    }

    fn create_buffer() -> paladinc::Result<paladinc::Buffer> {
        let simple = paladinc::SimpleBuffer::open("src/main.rs".into())?;

        #[derive(Clone)]
        struct Fake;

        impl LspResponseTransmitter for Fake {
            type Error = io::Error;

            fn send(&self, event: paladinc::lsp::LspResponse) -> Result<(), Self::Error> {
                // dbg!(event);

                Ok(())
            }
        }

        paladinc::Buffer::create(simple, ".".into(), Fake)
    }
}

impl Widget for BufferWidget {
    fn layout(&mut self, layout: Layout, font_system: &mut FontSystem) {
        self.text.layout(layout, font_system);
    }

    fn render(&self, layout: Layout, canvas: &mut Canvas) {
        self.text.render(layout, canvas)
    }
}

impl Element for BufferElement {
    fn insert(self, context: &mut impl paladin_view::InsertContext) {
        let mut qc = tree_sitter::QueryCursor::new();
        let query = tree_sitter::Query::new(
            &tree_sitter_rust::language(),
            tree_sitter_rust::HIGHLIGHT_QUERY,
        )
        .unwrap();

        let buffer = Self::create_buffer().unwrap();

        let content = get_rich_text_content(&buffer, 0, 149, &mut qc, &query);

        let text = Text::rich().text(content).size(32.0).call();

        let widget = BufferWidget {
            buffer,
            text,
            qc,
            query,
        };

        context.insert(paladin_view::MountedWidget::Custom(CustomWidget(Box::new(
            widget,
        ))));
    }

    fn compare_rebuild(
        self,
        old: paladin_view::MountedWidget,
        context: &mut impl paladin_view::RebuildContext,
    ) -> paladin_view::CompareResult<impl Element> {
        let paladin_view::MountedWidget::Custom(CustomWidget(custom)) = old else {
            return paladin_view::CompareResult::Replace { with: self };
        };

        let Ok(old) = custom.into_any().downcast::<BufferWidget>() else {
            return paladin_view::CompareResult::Replace { with: self };
        };

        if old.buffer.buffer.path.to_str() != Some(&self.path) {
            panic!("New path")
        }

        // no need to replace
        context.insert(paladin_view::MountedWidget::Custom(CustomWidget(old)));

        return paladin_view::CompareResult::Success;
    }
}

fn get_rich_text_content(
    editor_buffer: &paladinc::Buffer,
    start_line: usize,
    length: usize,
    ts_cursor: &mut tree_sitter::QueryCursor,
    query: &tree_sitter::Query,
) -> Vec<(String, cosmic_text::AttrsList)> {
    let now = std::time::Instant::now();
    let attrs = cosmic_text::Attrs::new().family(cosmic_text::Family::Name("JetBrains Mono"));

    let mut highlights = editor_buffer.highlight(ts_cursor, query, start_line..start_line + 80);

    let add_span = |list: &mut cosmic_text::AttrsList,
                    highlight: Option<highlight::LineHighlight>| {
        list.clear_spans();

        if let Some(highlight) = highlight {
            for e in highlight.into_iter() {
                let color = cosmic_text::Color::rgba(e.0.r, e.0.g, e.0.b, e.0.a);
                list.add_span(e.1.clone(), attrs.color(color));
            }
        }
    };

    let mut vec = vec![];

    for line in start_line..(start_line + length).min(editor_buffer.line_len()) {
        let mut attrs_list = cosmic_text::AttrsList::new(attrs);

        match highlights.current.cmp(&line) {
            // Trying to highlight a line that is before the text we are drawing now.
            std::cmp::Ordering::Less => {
                // Consume all the lines until we are where we want to be
                while highlights.current < line {
                    if let Some(highlight) = highlights.next_line() {
                        highlight.consume();
                    } else {
                        break;
                    }
                }

                add_span(&mut attrs_list, highlights.next_line());
            }
            std::cmp::Ordering::Equal => add_span(&mut attrs_list, highlights.next_line()),
            std::cmp::Ordering::Greater => {}
        };

        let text = editor_buffer.line(line).to_string();

        vec.push((text, attrs_list));
    }

    dbg!("Editor update took : {:?}", now.elapsed());

    vec
}

pub struct InitResult {
    pub workspace: PathBuf,
    pub file: Option<PathBuf>,
}

pub fn initial_workspace() -> miette::Result<InitResult> {
    let workspace = PathBuf::from("./").canonicalize().into_diagnostic()?;

    let mut args = std::env::args();
    let _ = args.next();

    let file = args.next();

    Ok(InitResult {
        workspace,
        file: file.map(Into::into),
    })
}
