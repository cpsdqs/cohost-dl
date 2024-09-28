use crate::data::{CommentDataV1, PostDataV1, ProjectDataV1};
use crate::post::{
    PostBlock, PostBlockAsk, PostBlockAskProject, PostBlockAttachment, PostBlockMarkdown,
};
use html5ever::{namespace_url, ns, QualName};
use kuchikiki::traits::TendrilSink;
use reqwest::Url;
use std::collections::HashSet;

pub trait ResourceRefs {
    fn collect_refs(&self, base: &Url) -> HashSet<Url>;
}

/// Strings are probably markdown
impl ResourceRefs for String {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();

        // our markdown rendering doesn't need to be accurate,
        // it just needs to parse images and such properly
        let parser = pulldown_cmark::Parser::new(&self);
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, parser);

        let frag = kuchikiki::parse_fragment(
            QualName::new(None, ns!(html), "body".into()),
            Default::default(),
        )
        .one(html);

        if let Ok(nodes) = frag.select("[style]") {
            for node in nodes {
                let attrs = node.attributes.borrow();
                let Some(style) = attrs.get("style") else {
                    continue;
                };

                let mut input = cssparser::ParserInput::new(style);
                let mut parser = cssparser::Parser::new(&mut input);

                while let Ok(tok) = parser.next() {
                    match tok {
                        cssparser::Token::UnquotedUrl(value) => {
                            if value.starts_with("data:") || value.is_empty() {
                                continue;
                            }

                            if let Ok(url) = base.join(value) {
                                if url.scheme() == "data" {
                                    continue;
                                }

                                refs.insert(url);
                            }
                        }
                        cssparser::Token::Function(f) if *f == "url" => {
                            let value: Result<_, cssparser::ParseError<()>> = parser
                                .parse_nested_block(|parser| {
                                    Ok(parser.expect_string().cloned()?)
                                });

                            if let Ok(value) = value {
                                if value.starts_with("data:") || value.is_empty() {
                                    continue;
                                }

                                if let Ok(url) = base.join(&value) {
                                    if url.scheme() == "data" {
                                        continue;
                                    }

                                    refs.insert(url);
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }

        if let Ok(nodes) = frag.select("img, source") {
            for node in nodes {
                let attrs = node.attributes.borrow();
                if let Some(src) = attrs.get("src") {
                    if !src.starts_with("data:") && !src.is_empty() {
                        if let Ok(src) = base.join(&src) {
                            refs.insert(src);
                        }
                    }
                }
                if let Some(src_set) = attrs.get("srcset") {
                    for url in urls_in_srcset(src_set) {
                        if !url.starts_with("data:") && !url.is_empty() {
                            if let Ok(src) = base.join(&url) {
                                refs.insert(src);
                            }
                        }
                    }
                }
            }
        }

        refs
    }
}

fn urls_in_srcset(s: &str) -> Vec<String> {
    // https://html.spec.whatwg.org/multipage/images.html#srcset-attributes
    struct Reader<'a> {
        chars: std::str::Chars<'a>,
        buf: Option<char>,
    }
    impl<'a> Reader<'a> {
        fn peek(&self) -> Option<char> {
            self.buf
        }
        fn next(&mut self) {
            self.buf = self.chars.next();
        }
    }
    let mut reader = Reader {
        chars: s.chars(),
        buf: None,
    };
    reader.next();

    let mut urls = Vec::new();

    while reader.peek().is_some() {
        // 1. whitespace*
        while reader.peek().map_or(false, |c| c.is_ascii_whitespace()) {
            reader.next();
        }
        // 2. URL, probably
        let mut url = String::new();
        if reader.peek() == Some(',') {
            // invalid
            continue;
        }
        while reader.peek().map_or(false, |c| !c.is_ascii_whitespace()) {
            if let Some(c) = reader.peek() {
                url.push(c);
            }
            reader.next();
        }
        if url.ends_with(',') {
            url.pop();
            urls.push(url);
            continue;
        }
        if !url.is_empty() {
            urls.push(url);
        }

        // 3. whitespace*
        while reader.peek().map_or(false, |c| c.is_ascii_whitespace()) {
            reader.next();
        }
        // 4. descriptor?
        while reader
            .peek()
            .map_or(false, |c| !c.is_ascii_whitespace() && c != ',')
        {
            reader.next();
        }
        // 5. whitespace*
        while reader.peek().map_or(false, |c| c.is_ascii_whitespace()) {
            reader.next();
        }
        // this should be a comma, but if not, well, whatever
        reader.next();
    }

    urls
}

#[test]
fn test_urls_in_srcset() {
    assert_eq!(
        urls_in_srcset(" https://example.com 3x, "),
        vec!["https://example.com".to_string()]
    );
    assert_eq!(
        urls_in_srcset(" https://example.com 3x, https://a.com/?a=1 , https://b.com"),
        vec![
            "https://example.com".to_string(),
            "https://a.com/?a=1".into(),
            "https://b.com".into()
        ]
    );
}

impl ResourceRefs for PostBlockMarkdown {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        self.content.collect_refs(base)
    }
}

impl ResourceRefs for PostBlockAttachment {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();
        match self {
            PostBlockAttachment::Image {
                file_url,
                preview_url,
                ..
            }
            | PostBlockAttachment::Audio {
                preview_url,
                file_url,
                ..
            } => {
                if !file_url.is_empty() {
                    if let Ok(file_url) = base.join(file_url) {
                        refs.insert(file_url);
                    }
                }
                if !preview_url.is_empty() {
                    if let Ok(preview_url) = base.join(preview_url) {
                        refs.insert(preview_url);
                    }
                }
            }
        }
        refs
    }
}

impl ResourceRefs for PostBlockAsk {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();

        if let Some(asking_project) = &self.asking_project {
            refs.extend(asking_project.collect_refs(base));
        }

        self.content.collect_refs(base);

        refs
    }
}

impl ResourceRefs for PostBlockAskProject {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();

        if let Ok(avatar_url) = base.join(&self.avatar_url) {
            refs.insert(avatar_url);
        }
        if let Ok(avatar_preview_url) = base.join(&self.avatar_preview_url) {
            refs.insert(avatar_preview_url);
        }

        refs
    }
}

impl ResourceRefs for PostBlock {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        match self {
            PostBlock::Ask { ask } => ask.collect_refs(base),
            PostBlock::Attachment { attachment } => attachment.collect_refs(base),
            PostBlock::AttachmentRow { attachments } => {
                let mut refs = HashSet::new();
                for attachment in attachments {
                    refs.extend(attachment.attachment.collect_refs(base));
                }
                refs
            }
            PostBlock::Markdown { markdown } => markdown.collect_refs(base),
        }
    }
}

impl ResourceRefs for PostDataV1 {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();

        for block in &self.blocks {
            refs.extend(block.collect_refs(base));
        }

        refs
    }
}

impl ResourceRefs for ProjectDataV1 {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        let mut refs = HashSet::new();

        if let Ok(avatar_url) = base.join(&self.avatar_url) {
            refs.insert(avatar_url);
        }
        if let Ok(avatar_preview_url) = base.join(&self.avatar_preview_url) {
            refs.insert(avatar_preview_url);
        }
        if let Some(header_url) = &self.header_url {
            if let Ok(header_url) = base.join(header_url) {
                refs.insert(header_url);
            }
        }
        if let Some(header_preview_url) = &self.header_preview_url {
            if let Ok(header_preview_url) = base.join(header_preview_url) {
                refs.insert(header_preview_url);
            }
        }

        refs.extend(self.description.collect_refs(base));
        refs
    }
}

impl ResourceRefs for CommentDataV1 {
    fn collect_refs(&self, base: &Url) -> HashSet<Url> {
        self.body.collect_refs(base)
    }
}
