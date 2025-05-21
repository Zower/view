#![feature(precise_capturing_in_traits)]

use std::{io, path::PathBuf};

use bevy_reflect::TypeRegistry;
use components::root::Root;

use cosmic_text::FontSystem;
use miette::IntoDiagnostic;
use paladin_view::{
    prelude::*, BuildResult, CustomWidget, InsertChildren, LeafNode, RebuildChildren, Style,
    Styleable,
};
use paladinc::{lsp::LspResponseTransmitter, ts::highlight};
mod components;

fn main() -> paladin_view::Result<()> {
    run(Root)
}

pub struct BufferElement {
    path: String,
    style: Style,
}

struct BufferWidget {
    buffer: paladinc::Buffer,
    text: paladin_view::Text,
    qc: tree_sitter::QueryCursor,
    query: tree_sitter::Query,
    style: Style,
}

impl BufferElement {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            style: Default::default(),
        }
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

    fn style(&self) -> Style {
        self.style.clone()
    }
}

impl Element for BufferElement {
    fn create(self, _: &mut TypeRegistry) -> BuildResult<impl InsertChildren> {
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
            style: self.style,
        };

        BuildResult {
            widget: paladin_view::MountedWidget::Custom(CustomWidget(Box::new(widget))),
            children: None::<LeafNode>,
        }
    }

    fn compare_rebuild(
        self,
        old: paladin_view::MountedWidget,
    ) -> paladin_view::BuildResult<impl RebuildChildren> {
        let paladin_view::MountedWidget::Custom(CustomWidget(custom)) = old else {
            panic!()
        };

        let Ok(old) = custom.into_any().downcast::<BufferWidget>() else {
            panic!()
        };

        // if old.buffer.buffer.path.to_str() != Some(&self.path) {
        //     panic!("New path")
        // }

        // no need to replace
        BuildResult {
            widget: paladin_view::MountedWidget::Custom(CustomWidget(old)),
            children: None::<LeafNode>,
        }
    }
}

impl Styleable for BufferElement {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
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
