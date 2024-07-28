use anyhow::bail;
use pulldown_cmark::{Event, HeadingLevel, Tag, TagEnd};

use crate::source::{RepoIdent, RepoLink};

pub fn parse_markdown(input: &str) -> Result<Vec<RepoLink>, anyhow::Error> {
    let mut ctx = ParseContext {
        section: Vec::new(),
        links: Vec::new(),
    };

    let mut iter = pulldown_cmark::TextMergeStream::new(pulldown_cmark::Parser::new(input));
    while let Some(()) = parse_event(&mut ctx, &mut iter)? {}

    Ok(ctx.links)
}

struct ParseContext {
    section: Vec<String>,
    links: Vec<RepoLink>,
}

fn parse_event<'a, I>(ctx: &mut ParseContext, iter: &mut I) -> Result<Option<()>, anyhow::Error>
where
    I: Iterator<Item = Event<'a>>,
{
    let Some(ev) = iter.next() else {
        return Ok(None);
    };

    match ev {
        Event::Start(Tag::Heading { level, .. }) => {
            let lvl = level as usize - 1;

            if level == HeadingLevel::H1 {
                // Ignore h1
            } else {
                let content = parse_content(TagEnd::Heading(level), iter)?;

                if ctx.section.len() < lvl {
                    ctx.section.push(content);
                } else {
                    ctx.section.truncate(lvl - 1);
                    ctx.section.push(content);
                }
            }
        }
        Event::Start(Tag::Link {
            link_type: _,
            dest_url,
            title: _,
            id: _,
        }) => {
            let _content = parse_content(TagEnd::Link, iter)?;
            if let Ok(ident) = RepoIdent::parse_url(&dest_url) {
                let link = RepoLink {
                    ident,
                    section: ctx.section.clone(),
                };
                ctx.links.push(link);
            }
        }
        Event::Start(_) => {}
        Event::End(_) => {}
        Event::Text(_) => {}
        Event::Code(_) => {}
        Event::InlineMath(_) => {}
        Event::DisplayMath(_) => {}
        Event::Html(_) => {}
        Event::InlineHtml(_) => {}
        Event::FootnoteReference(_) => {}
        Event::SoftBreak => {}
        Event::HardBreak => {}
        Event::Rule => {}
        Event::TaskListMarker(_) => {}
    }

    Ok(Some(()))
}

fn parse_content<'a>(
    tag: TagEnd,
    iter: &mut impl Iterator<Item = Event<'a>>,
) -> Result<String, anyhow::Error> {
    let mut buffer = String::new();

    while let Some(ev) = iter.next() {
        match ev {
            Event::Start(tag) => {
                // Skip

                let end = match tag {
                    Tag::Paragraph => TagEnd::Paragraph,
                    Tag::Heading { level, .. } => TagEnd::Heading(level),
                    Tag::BlockQuote(_) => TagEnd::BlockQuote,
                    Tag::CodeBlock(_) => TagEnd::CodeBlock,
                    Tag::HtmlBlock => TagEnd::HtmlBlock,
                    Tag::List(ordered) => TagEnd::List(ordered.is_some()),
                    Tag::Item => TagEnd::Item,
                    Tag::FootnoteDefinition(_) => TagEnd::FootnoteDefinition,
                    Tag::Table(_) => TagEnd::Table,
                    Tag::TableHead => TagEnd::TableHead,
                    Tag::TableRow => TagEnd::TableRow,
                    Tag::TableCell => TagEnd::TableCell,
                    Tag::Emphasis => TagEnd::Emphasis,
                    Tag::Strong => TagEnd::Strong,
                    Tag::Strikethrough => TagEnd::Strikethrough,
                    Tag::Link { .. } => TagEnd::Link,
                    Tag::Image { .. } => TagEnd::Image,
                    Tag::MetadataBlock(kind) => TagEnd::MetadataBlock(kind),
                };
                // TODO: use inner content?
                let _ = parse_content(end, iter)?;
            }
            Event::End(t) => {
                if t == tag {
                    break;
                } else {
                    bail!("could not parse content: unexpected end tag {t:?}");
                }
            }
            Event::Text(text) => {
                if !buffer.is_empty() {
                    buffer.push(' ');
                }
                buffer.push_str(&text);
            }
            Event::Code(_code) => {}
            Event::InlineMath(_math) => {}
            Event::DisplayMath(_math) => {}
            Event::Html(_html) => {}
            Event::InlineHtml(_html) => {}
            Event::FootnoteReference(_reference) => {}
            Event::SoftBreak => {}
            Event::HardBreak => {}
            Event::Rule => {}
            Event::TaskListMarker(_) => {}
        }
    }

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_basic() {
        let input = r#"
# main

## hello

[repo](https://github.com/a/a)

## world

[repo](https://github.com/a/b)

"#;
        let out = parse_markdown(input).unwrap();
        assert_eq!(
            out,
            vec![
                RepoLink {
                    ident: RepoIdent::new_github("a", "a"),
                    section: vec!["hello".to_string()]
                },
                RepoLink {
                    ident: RepoIdent::new_github("a", "b"),
                    section: vec!["world".to_string()]
                }
            ]
        );
    }
}
