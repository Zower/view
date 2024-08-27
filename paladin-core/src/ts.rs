use crop::Rope;
use tree_sitter::{Parser, Tree};

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

pub fn tree(source: &Rope, old_tree: Option<&Tree>) -> Tree {
    let mut parser = Parser::new();

    parser.set_language(&tree_sitter_rust::language()).unwrap();

    parser
        .parse_with(
            &mut |a, _| {
                let mut str = source.byte_slice(a..).chunks();

                str.next().unwrap_or("")
            },
            old_tree,
        )
        .unwrap()
}

pub mod highlight {
    use std::{collections::HashMap, iter::Peekable, ops::Range};

    use super::Color;
    use crop::{Rope, RopeSlice};
    use tree_sitter::{Query, QueryCaptures, QueryCursor, TextProvider, Tree};

    pub fn syntax_highlight<'query, 'tree: 'query, 'rope>(
        tree: &'tree Tree,
        cursor: &'query mut QueryCursor,
        query: &'query Query,
        source: &'rope Rope,
        range: std::ops::Range<usize>,
    ) -> LineHighlights<'query, 'tree, 'rope> {
        let source = source.byte_slice(..);

        cursor.set_point_range(std::ops::Range {
            start: tree_sitter::Point {
                row: range.start,
                column: 0,
            },
            end: tree_sitter::Point {
                row: range.end,
                column: usize::MAX,
            },
        });

        let provider = RopeTextProvider { inner: source };
        let root_node = tree.root_node();
        let captures = cursor.captures(query, root_node, provider);

        let mut map = HashMap::with_hasher(ahash::RandomState::new());

        map.insert("constructor", Color::rgb(60, 69, 112));
        map.insert("function", Color::rgb(234, 184, 120));
        map.insert("function.method", Color::rgb(234, 184, 120));
        map.insert("function.macro", Color::rgb(234, 184, 120));
        map.insert("keyword", Color::rgb(204, 139, 96));
        map.insert("punctuation.delimiter", Color::rgb(204, 139, 96));
        map.insert("punctuation.bracket", Color::rgb(255, 255, 255));
        map.insert("type", Color::rgb(60, 69, 112));
        map.insert("type.builtin", Color::rgb(60, 69, 112));
        map.insert("property", Color::rgb(130, 130, 200));
        map.insert("string", Color::rgb(149, 175, 97));
        map.insert("operator", Color::rgb(204, 139, 96));
        map.insert("variable.builtin", Color::rgb(60, 69, 112));
        map.insert("variable.parameter", Color::rgb(60, 69, 112));
        map.insert("comment", Color::rgb(128, 128, 128));
        map.insert("constant.builtin", Color::rgb(212, 252, 182));
        map.insert("escape", Color::rgb(113, 10, 250));
        map.insert("attribute", Color::rgb(219, 211, 186));
        map.insert("label", Color::rgb(134, 173, 199));

        let mut inner = captures.peekable();

        let byte = inner
            .peek()
            .map(|it| it.0.captures[0].node.start_byte())
            .unwrap_or(0);

        let line = if byte <= source.byte_len() {
            source.line_of_byte(byte)
        } else {
            0
        };

        LineHighlights {
            source,
            inner,
            names: query.capture_names(),
            current: line,
            map,
        }
    }

    pub struct LineHighlights<'query, 'tree: 'query, 'rope> {
        pub source: RopeSlice<'rope>,
        pub inner: Peekable<QueryCaptures<'query, 'tree, RopeTextProvider<'rope>, &'rope [u8]>>,
        pub names: &'query [&'query str],
        pub current: usize,
        pub map: HashMap<&'static str, Color, ahash::RandomState>,
    }

    impl<'query, 'tree: 'query, 'rope> LineHighlights<'query, 'tree, 'rope> {
        pub fn next_line(&'_ mut self) -> Option<LineHighlight<'_, 'query, 'tree, 'rope>> {
            let _ = self.inner.peek()?;

            Some(LineHighlight { iter: self })
        }
    }

    pub struct LineHighlight<'parent, 'query, 'tree, 'rope> {
        pub iter: &'parent mut LineHighlights<'query, 'tree, 'rope>,
    }

    impl<'query, 'tree, 'rope> LineHighlight<'_, 'query, 'tree, 'rope> {
        pub fn consume(self) {
            for _ in self.into_iter() {}
        }
    }

    impl<'query, 'tree, 'rope> Iterator for LineHighlight<'_, 'query, 'tree, 'rope> {
        type Item = (Color, Range<usize>);

        fn next(&mut self) -> Option<Self::Item> {
            let (capture, idx) = self.iter.inner.peek()?;

            let node = capture.captures[*idx].node;

            // TODO: always same line?
            // Answer: No.
            // Multiline strings.
            let line1 = self.iter.source.line_of_byte(node.start_byte());
            let line2 = self.iter.source.line_of_byte(node.end_byte());

            let start = self
                .iter
                .source
                .byte_of_line(self.iter.source.line_of_byte(node.start_byte()));

            assert!(line1 >= self.iter.current);

            if line1 != line2 {
                let range = if self.iter.current == line1 {
                    node.start_byte() - start..self.iter.source.line(line1).byte_len()
                } else if self.iter.current < line2 {
                    0..self.iter.source.line(self.iter.current).byte_len()
                } else {
                    let start = self
                        .iter
                        .source
                        .byte_of_line(self.iter.source.line_of_byte(node.end_byte()));

                    0..node.end_byte() - start
                };

                // TODO: this doesn't really work.
                // The capture should still be available to the next lines lines?
                let (capture, idx) = self.iter.inner.next().unwrap();

                let capture = capture.captures[idx];

                let kind = self.iter.names.get(capture.index as usize).unwrap();

                let color = *self.iter.map.get(kind).unwrap();

                self.iter.current += 1;

                return Some((color, range));
            }

            debug_assert_eq!(line1, line2);

            // not meant for us
            if line2 > self.iter.current {
                self.iter.current += 1;

                return None;
            }

            // It's for us, get it
            let (capture, idx) = self.iter.inner.next().unwrap();

            let capture = capture.captures[idx];

            let kind = self.iter.names.get(capture.index as usize).unwrap();

            let color = *self.iter.map.get(kind).unwrap_or(&Color::rgb(255, 0, 0));

            let range = (node.start_byte() - start)..node.end_byte() - start;

            Some((color, range))
        }
    }

    pub struct RopeTextProvider<'a> {
        pub inner: RopeSlice<'a>,
    }

    pub struct ChunksBytes<'a> {
        inner: crop::iter::Chunks<'a>,
    }

    impl<'a> Iterator for ChunksBytes<'a> {
        type Item = &'a [u8];

        fn next(&mut self) -> Option<Self::Item> {
            self.inner.next().map(str::as_bytes)
        }
    }

    impl<'rope> TextProvider<&'rope [u8]> for RopeTextProvider<'rope> {
        type I = ChunksBytes<'rope>;

        fn text(&mut self, node: tree_sitter::Node) -> Self::I {
            ChunksBytes {
                inner: self.inner.byte_slice(node.byte_range()).chunks(),
            }
        }
    }
}
