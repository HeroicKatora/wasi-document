use core::{error::Error, ops};
use lithtml::{Dom, Node};

pub struct Structure {
    pub html_tag: TagSpan,
    pub html_insertion_point: usize,
    pub insertion_tag: TagSpan,
    pub stage0: TagSpan,
}

#[derive(Clone, Copy)]
pub struct TagSpan {
    pub start: SourceCharacter,
    pub end: SourceCharacter,
}

#[derive(Clone, Copy)]
pub struct SourceCharacter {
    pub line: usize,
    pub column: usize,
}

pub struct SourceDocument<'text> {
    text: &'text str,
    by_line: Vec<usize>,
}

fn parse_tar_tags(source: &SourceDocument, doc: &str) -> Result<Structure, Box<dyn Error>> {
    const ID_TAR_CONTENT: &str = "WAH_POLYGLOT_HTML_PLUS_TAR_CONTENT";
    const ID_TAR_STAGE0: &str = "WAH_POLYGLOT_HTML_PLUS_TAR_STAGE0";

    let dom = Dom::parse(doc)?;

    let html = find_element(&dom, |node| {
        node.element().filter(|el| el.name.to_lowercase() == "html")
    })
    .ok_or_else(|| no_node("begin of Tar file", "starting `<html>` tag"))?;

    let html_insertion_point = source.element_end_of_start_tag(html);

    let insertion = find_element(&dom, |node| {
        node.element()
            .filter(|el| el.id.as_deref() == Some(ID_TAR_CONTENT))
    })
    .ok_or_else(|| {
        no_node(
            "tag marked as insertion point for tar contents",
            &format!("tag with id `{}`", ID_TAR_CONTENT),
        )
    })?;

    let stage0 = find_element(&dom, |node| {
        node.element()
            .filter(|el| el.name.to_lowercase() == "script")
            .filter(|el| el.id.as_deref() == Some(ID_TAR_STAGE0))
    })
    .ok_or_else(|| {
        no_node(
            "tag marked as insertion point for script entry point",
            &format!("`<script>` tag with id `{}`", ID_TAR_STAGE0),
        )
    })?;

    #[derive(Debug)]
    struct MissingNodeError {
        content: String,
        searched_for: String,
    }

    impl core::fmt::Display for MissingNodeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "Missing Node to insert {}, searched for {}",
                self.content, self.searched_for,
            )
        }
    }

    impl Error for MissingNodeError {}

    fn no_node(name: &str, searched: &str) -> Box<dyn Error> {
        Box::new(MissingNodeError {
            content: name.to_string(),
            searched_for: searched.to_string(),
        })
    }

    Ok(Structure {
        html_tag: html.into(),
        html_insertion_point,
        insertion_tag: insertion.into(),
        stage0: stage0.into(),
    })
}

fn find_element<'a, T>(dom: &'a Dom, mut with: impl FnMut(&'a Node) -> Option<T>) -> Option<T> {
    let mut stack: Vec<_> = dom.children.iter().collect();

    while let Some(top) = stack.pop() {
        if let Some(find) = with(top) {
            return Some(find);
        }

        let children = top.element().into_iter().flat_map(|el| el.children.iter());
        stack.extend(children);
    }

    None
}

impl<'text> SourceDocument<'text> {
    pub fn new(text: &'text str) -> Self {
        let by_line = text.split_inclusive('\n').scan(0usize, |acc, val| {
            let start = *acc;
            *acc += val.len();
            Some(start)
        });

        SourceDocument {
            text,
            by_line: Vec::from_iter(by_line),
        }
    }

    pub fn span(&self, span: TagSpan) -> ops::Range<usize> {
        // FIXME: unsure if the `column` attribute is by character or byte offset.
        let start = self.by_line[span.start.line.checked_sub(1).unwrap()]
            + span.start.column.checked_sub(1).unwrap();
        let end = self.by_line[span.end.line.checked_sub(1).unwrap()]
            + span.end.column.checked_sub(1).unwrap();

        start..end
    }

    pub fn element_end_of_start_tag(&self, el: &lithtml::Element) -> usize {
        let span: TagSpan = el.into();

        let non_ending_leq = el
            .attributes
            .keys()
            .chain(el.attributes.values().flat_map(|opt| opt.as_ref()))
            .flat_map(|st| st.chars())
            .filter(|&ch| ch == '>')
            .count();

        let outer_html = &self[self.span(span)];

        let (closing_leq, _) = outer_html
            .char_indices()
            .filter(|&(_, ch)| ch == '>')
            .nth(non_ending_leq)
            .expect("html opening tag not closed?");

        closing_leq + '>'.len_utf8()
    }

    pub fn html_tar_structure(&self) -> Result<Structure, Box<dyn Error>> {
        parse_tar_tags(self, self.text)
    }
}

impl<'text> ops::Index<ops::Range<usize>> for SourceDocument<'text> {
    type Output = str;

    fn index(&self, index: ops::Range<usize>) -> &Self::Output {
        &self.text[index]
    }
}

impl<'text> ops::Index<ops::RangeFrom<usize>> for SourceDocument<'text> {
    type Output = str;

    fn index(&self, index: ops::RangeFrom<usize>) -> &Self::Output {
        &self.text[index]
    }
}

impl<'text> ops::Index<ops::RangeFull> for SourceDocument<'text> {
    type Output = str;

    fn index(&self, _: ops::RangeFull) -> &Self::Output {
        self.text
    }
}

impl From<&'_ lithtml::Element> for TagSpan {
    fn from(el: &'_ lithtml::Element) -> Self {
        TagSpan {
            start: SourceCharacter {
                line: el.source_span.start_line,
                column: el.source_span.start_column,
            },
            end: SourceCharacter {
                line: el.source_span.end_line,
                column: el.source_span.end_column,
            },
        }
    }
}
