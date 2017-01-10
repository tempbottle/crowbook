// Copyright (C) 2016-2017 Élisabeth HENRY.
//
// This file is part of Crowbook.
//
// Crowbook is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published
// by the Free Software Foundation, either version 2.1 of the License, or
// (at your option) any later version.
//
// Caribon is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Lesser General Public License for more details.
//
// You should have received ba copy of the GNU Lesser General Public License
// along with Crowbook.  If not, see <http://www.gnu.org/licenses/>.

use error::{Result, Error, Source};
use token::Token;
use token::Data;
use book::{Book, compile_str};
use number::Number;
use resource_handler::ResourceHandler;
use renderer::Renderer;
use parser::Parser;
use syntax::Syntax;
use logger::Logger;
use lang;

use std::borrow::Cow;
use std::convert::{AsMut, AsRef};

use crowbook_text_processing::escape;
use numerals::roman::Roman;
use epub_builder::Toc;
use epub_builder::TocElement;

#[derive(Debug, PartialEq, Copy, Clone)]
/// If/how to highlight code
pub enum Highlight {
    None,
    Js,
    Syntect,
}
    

/// Base structure for rendering HTML files
///
/// Used by EpubRenderer, HtmlSingleRenderer, HtmlDirRenderer
pub struct HtmlRenderer<'a> {
    table_head: bool,
    #[doc(hidden)]
    pub verbatim: bool,
    current_par: u32,
    #[doc(hidden)]
    pub first_letter: bool,
    first_paragraph: bool,
    footnotes: Vec<(String, String)>,
    filename: String,

    /// Book that must be rendered
    pub book: &'a Book,

    /// Proofread or not
    pub proofread: bool,

    /// Current part, chapter (and subsection, subsubsection and so on)
    #[doc(hidden)]
    pub current_chapter: [i32; 7],

    /// Current numbering level
    #[doc(hidden)]
    pub current_numbering: i32,

    /// Whether current chapter's title must be displayed
    #[doc(hidden)]
    pub current_hide: bool,

    /// Whether current chapter is actually a part
    #[doc(hidden)]
    pub current_part: bool,

    /// Resource handler
    #[doc(hidden)]
    pub handler: ResourceHandler<'a>,

    /// Current footnote number
    #[doc(hidden)]
    pub footnote_number: u32,

    /// Source for error messages
    #[doc(hidden)]
    pub source: Source,

    /// Table of contents
    #[doc(hidden)]
    pub toc: Toc,

    #[doc(hidden)]
    pub highlight: Highlight,

    /// Current link number
    #[doc(hidden)]
    pub link_number: u32,

    syntax: Option<Syntax>,
}

impl<'a> HtmlRenderer<'a> {
    fn get_highlight(book: &Book) -> (Highlight, Option<Syntax>) {
        match book.options.get_str("rendering.highlight").unwrap() {
            "syntect" => (Highlight::Syntect, Some(Syntax::new())),
            "none" => (Highlight::None, None),
            "highlight.js" => (Highlight::Js, None),
            value => {
                Logger::display_error(lformat!("rendering.highlight set to '{}', not a valid value",
                                                   value));
                (Highlight::None, None)
            }
        }
    }

    /// Creates a new HTML renderer
    pub fn new(book: &'a Book) -> HtmlRenderer<'a> {
        let (highlight, syntax) = Self::get_highlight(book);

        let mut html = HtmlRenderer {
            book: book,
            toc: Toc::new(),
            link_number: 0,
            current_chapter: [0, 0, 0, 0, 0, 0, 0],
            current_numbering: book.options.get_i32("rendering.num_depth").unwrap(),
            current_part: false,
            current_par: 0,
            current_hide: false,
            table_head: false,
            footnote_number: 0,
            footnotes: vec![],
            verbatim: false,
            filename: String::new(),
            handler: ResourceHandler::new(&book.logger),
            source: Source::empty(),
            first_letter: false,
            first_paragraph: true,
            proofread: false,
            syntax: syntax,
            highlight: highlight,
        };
        html.handler.set_images_mapping(true);
        html.handler.set_base64(true);
        html
    }

    /// Add a footnote which will be renderer later on
    #[doc(hidden)]
    pub fn add_footnote(&mut self, number: String, content: String) {
        self.footnotes.push((number, content));
    }

    /// Configure the Renderer for this chapter
    #[doc(hidden)]
    pub fn chapter_config(&mut self, i: usize, n: Number, filename: String) {
        self.source = Source::new(self.book.chapters[i].filename.as_str());
        self.first_paragraph = true;
        self.current_hide = false;
        let book_numbering = self.book.options.get_i32("rendering.num_depth").unwrap();
        match n {
            Number::Unnumbered | Number::UnnumberedPart => self.current_numbering = 0,
            Number::Default | Number::DefaultPart => self.current_numbering = book_numbering,
            Number::Specified(n) => {
                self.current_numbering = book_numbering;
                self.current_chapter[1] = n - 1;
            },
            Number::SpecifiedPart(n) => {
                self.current_numbering = book_numbering;
                self.current_chapter[0] = n - 1;
            }
            Number::Hidden => {
                self.current_numbering = 0;
                self.current_hide = true;
            },
        } //          _ => panic!("Parts are not supported yet"),
        self.current_part = n.is_part();
        
        self.filename = filename;
    }

    /// Renders a chapter to HTML
    pub fn render_html<T>(this: &mut T, tokens: &[Token], render_end_notes: bool) -> Result<String>
        where T: AsMut<HtmlRenderer<'a>> + AsRef<HtmlRenderer<'a>> + Renderer
    {
        let mut res = String::new();
        for token in tokens {
            res.push_str(&this.render_token(token)?);
            this.as_mut().render_side_notes(&mut res);
        }
        if render_end_notes {
            this.as_mut().render_end_notes(&mut res);
        }
        Ok(res)
    }

    /// Renders a title (without `<h1>` tags), increasing header number beforehand
    #[doc(hidden)]
    pub fn render_title(&mut self, n: i32, vec: &[Token]) -> Result<String> {
        let n = if self.current_part {
            n -1
        } else {
            n
        };
        self.inc_header(n);
        let s = if n == 1 && self.current_numbering >= 1 {
            let chapter = self.current_chapter[1];
            let c_title = self.render_vec(vec)?;
            let res = self.book.get_chapter_header(chapter, c_title, |s| {
                self.render_vec(&Parser::new().parse_inline(s)?)
            });
            res?
        } else if self.current_numbering >= n {
            format!("{} {}", self.get_numbers(), self.render_vec(vec)?)
        } else {
            self.render_vec(vec)?
        };
        Ok(s)
    }

    /// Renders a title, including `<h1>` tags and appropriate links
    #[doc(hidden)]
    pub fn render_title_full(&mut self, n: i32, inner: String) -> Result<String> {
        if n == 1 && self.current_hide {
            Ok(format!("<h1 id = \"link-{}\"></h1>", self.link_number))
        } else if n == 1 && self.current_part {
            let part = self.current_chapter[0];
            let res = self.book.get_part_header(part, |s| {
                self.render_vec(&Parser::new().parse_inline(s)?)
            })?;
            Ok(format!("<h2 class = \"part\">{}</h2>
<h1 id = \"link-{}\" class = \"part\">{}</h1>\n",
                    res,
                    self.link_number,
                    inner))
        } else {
            Ok(format!("<h{} id = \"link-{}\">{}</h{}>\n",
                    n,
                    self.link_number,
                    inner,
                    n))
        }
    }

    /// Increases a header if it needs to be
    ///
    /// Also sets up first_paragraph, link stuff and so on
    fn inc_header(&mut self, n: i32) {
        if n <= 1 {
            self.first_paragraph = true;
        }
        if self.current_numbering >= n {
            assert!(n >= 0);
            let n = n as usize;
            assert!(n < self.current_chapter.len());
            self.current_chapter[n] += 1;
            let begin = if n == 0 && !self.book.options.get_bool("rendering.part.reset_counter").unwrap() {
                n + 2
            } else {
                n + 1
            };
            for i in begin..self.current_chapter.len() {
                self.current_chapter[i] = 0;
            }
        }
        self.link_number += 1;
    }

    /// Returns a "x.y.z" corresponding to current chapter/section/...
    fn get_numbers(&self) -> String {
        let mut output = String::new();
        for i in 1..self.current_chapter.len() {
            if self.current_chapter[i] == 0 {
                if i == self.current_chapter.len() - 1 {
                    break;
                }
                let bools: Vec<_> = self.current_chapter[i + 1..].iter().map(|x| *x != 0).collect();
                if !bools.contains(&true) {
                    break;
                }
            }
            if i != 1 || !self.book.options.get_bool("rendering.chapter.roman_numerals").unwrap() {
                output.push_str(&format!("{}.", self.current_chapter[i])); //todo
            } else if self.current_chapter[i] >= 1 {
                output.push_str(&format!("{:X}.", Roman::from(self.current_chapter[i] as i16)));
            } else {
                self.book.logger.error(lformat!("can not use roman numerals with zero or negative chapter numbers ({n})",
                                                    n = self.current_chapter[i]));
            }
        }
        output
    }


    /// Display side notes if option is to true
    #[doc(hidden)]
    pub fn render_side_notes(&mut self, res: &mut String) {
        if self.book.options.get_bool("html.side_notes").unwrap() {
            for (note_number, footnote) in self.footnotes.drain(..) {
                res.push_str(&format!("<div class = \"sidenote\">\n{} {}\n</div>\n",
                                      note_number,
                                      footnote));
            }
        }
    }

    /// Display end notes, if side_notes option is set to false
    #[doc(hidden)]
    pub fn render_end_notes(&mut self, res: &mut String) {
        if !self.footnotes.is_empty() {

            //             for (note_number, footnote) in self.footnotes.drain(..) {
            //                 res.push_str(&format!("<div class = \"note\">
            //  <p>{}</p>
            // {}
            // </div>\n",
            //                                       note_number,
            //                                       footnote));
            //             }


            res.push_str(&format!("<div class = \"notes\">
 <h2 class = \"notes\">{}</h2>\n",
                                  lang::get_str(self.book.options.get_str("lang").unwrap(),
                                                "notes")));
            res.push_str("<table class = \"notes\">\n");
            for (note_number, footnote) in self.footnotes.drain(..) {
                res.push_str(&format!("<tr class = \"notes\">
 <td class = \"note-number\">
  {}
 </td>
 <td class = \"note\">
  {}
  </td>
</tr>\n",
                                      note_number,
                                      footnote));
            }
            res.push_str("</table>\n");
            res.push_str("</div>\n");
        }
    }

    /// Renders a token
    ///
    /// Used by render_token implementation of Renderer trait. Separate function
    /// because we need to be able to call it from other renderers.
    ///
    /// See http://lise-henry.github.io/articles/rust_inheritance.html
    #[doc(hidden)]
    pub fn static_render_token<T>(this: &mut T, token: &Token) -> Result<String>
        where T: AsMut<HtmlRenderer<'a>> + AsRef<HtmlRenderer<'a>> + Renderer
    {
        match *token {
            Token::Annotation(ref annotation, ref v) => {
                let content = this.as_mut().render_vec(v)?;
                if this.as_ref().proofread {
                    match *annotation {
                        Data::GrammarError(ref s) => {
                            Ok(format!("<span title = \"{}\" class = \"grammar-error\">{}</span>",
                                       escape::quotes(s.as_str()),
                                       content))
                        }
                        Data::Repetition(ref colour) => {
                            if !this.as_ref().verbatim {
                                Ok(format!("<span class = \"repetition\" \
                                            style = \"text-decoration-line: underline; \
                                            text-decoration-style: wavy; \
                                            text-decoration-color: {colour}\">{content}</span>",
                                           colour = colour,
                                           content = content))
                            } else {
                                Ok(content)
                            }
                        },
                        _ => unreachable!(),
                    }
                } else {
                    Ok(content)
                }
            }
            Token::Str(ref text) => {
                let mut content = if this.as_ref().verbatim {
                    Cow::Borrowed(text.as_ref())
                } else {
                    escape::html(this.as_ref().book.clean(text.as_ref(), false))
                };
                if this.as_ref().first_letter {
                    this.as_mut().first_letter = false;
                }

                if this.as_ref().book.options.get_bool("html.escape_nb_spaces").unwrap() {
                    content = escape::nb_spaces(content);
                }
                Ok(content.into_owned())
            }
            Token::Paragraph(ref vec) => {
                if this.as_ref().first_paragraph {
                    this.as_mut().first_paragraph = false;
                    if !vec.is_empty() && vec[0].is_str() {
                        // Only use initials if first element is a Token::str
                        this.as_mut().first_letter = true;
                    }
                }
                let class = if this.as_ref().first_letter &&
                               this.as_ref().book.options.get_bool("rendering.initials").unwrap() {
                    " class = \"first-para\""
                } else {
                    ""
                };
                let content = this.render_vec(vec)?;
                this.as_mut().current_par += 1;
                let par = this.as_ref().current_par;
                Ok(format!("<p id = \"para-{}\"{}>{}</p>\n", par, class, content))
            }
            Token::Header(n, ref vec) => {
                let s = this.as_mut().render_title(n, vec)?;
                if n <= this.as_ref().book.options.get_i32("rendering.num_depth").unwrap() {
                    let url = format!("{}#link-{}",
                                      this.as_ref().filename,
                                      this.as_ref().link_number);
                    if !this.as_ref().current_part {
                        this.as_mut().toc.add(TocElement::new(url, s.clone())
                                              .level(n));

                    } else {
                        let is_roman = this.as_ref().book.options.get_bool("rendering.part.roman_numerals").unwrap();
                        let number = this.as_ref().current_chapter[0];
                        let number = if is_roman && number >= 1 {
                            format!("{:X}", Roman::from(number as i16))
                        } else {
                            format!("{}", number)
                        };
                        let toc_content = format!("{number} {content}",
                                                  content = s,
                                                  number = number);
                        this.as_mut().toc.add(TocElement::new(url, toc_content)
                                              .level(n - 1));
                    }
                }
                Ok(this.as_mut().render_title_full(n, s)?)
            }
            Token::Emphasis(ref vec) => Ok(format!("<em>{}</em>", this.render_vec(vec)?)),
            Token::Strong(ref vec) => Ok(format!("<b>{}</b>", this.render_vec(vec)?)),
            Token::Code(ref vec) => Ok(format!("<code>{}</code>", this.render_vec(vec)?)),
            Token::BlockQuote(ref vec) => {
                Ok(format!("<blockquote>{}</blockquote>\n", this.render_vec(vec)?))
            }
            Token::CodeBlock(ref language, ref vec) => {
                this.as_mut().verbatim = true;
                let s = this.render_vec(vec)?;
                let output = if let Some(ref syntax) = this.as_ref().syntax {
                    syntax.to_html(&s, language)
                } else if language.is_empty() {
                    format!("<pre><code>{}</code></pre>\n", s)
                } else {
                    format!("<pre><code class = \"language-{}\">{}</code></pre>\n",
                            language,
                            escape::html(s))
                };
                this.as_mut().verbatim = false;
                Ok(output)
            }
            Token::Rule => Ok(String::from("<p class = \"rule\">***</p>\n")),
            Token::SoftBreak => Ok(String::from(" ")),
            Token::HardBreak => Ok(String::from("<br />\n")),
            Token::List(ref vec) => Ok(format!("<ul>\n{}</ul>\n", this.render_vec(vec)?)),
            Token::OrderedList(n, ref vec) => {
                Ok(format!("<ol{}>\n{}</ol>\n",
                           if n == 1 {
                               String::new()
                           } else {
                               format!(" start = \"{}\"", n)
                           },
                           this.render_vec(vec)?))
            }
            Token::Item(ref vec) => Ok(format!("<li>{}</li>\n", this.render_vec(vec)?)),
            Token::Link(ref url, ref title, ref vec) => {
                let url = escape::html(url.as_ref());
                let url = if ResourceHandler::is_local(&url) {
                    Cow::Owned(this.as_ref().handler.get_link(&url).to_owned())
                } else {
                    url
                };

                Ok(format!("<a href = \"{}\"{}>{}</a>",
                           url,
                           if title.is_empty() {
                               String::new()
                           } else {
                               format!(" title = \"{}\"", title)
                           },
                           this.render_vec(vec)?))
            }
            Token::Image(ref url, ref title, ref alt) |
            Token::StandaloneImage(ref url, ref title, ref alt) => {
                let content = this.render_vec(alt)?;
                let html: &mut HtmlRenderer = this.as_mut();
                let url = html.handler.map_image(&html.source, url.as_ref())?;

                if token.is_image() {
                    Ok(format!("<img src = \"{}\" title = \"{}\" alt = \"{}\" />",
                               url,
                               title,
                               content))
                } else {
                    Ok(format!("<div class = \"image\">
  <img src = \"{}\" title = \"{}\" alt = \
                                \"{}\" />
</div>",
                               url,
                               title,
                               content))
                }
            }
            Token::Table(_, ref vec) => {
                Ok(format!("<div class = \"table\">
    <table>\n{}
    </table>
</div>\n",
                           this.render_vec(vec)?))
            }
            Token::TableRow(ref vec) => Ok(format!("<tr>\n{}</tr>\n", this.render_vec(vec)?)),
            Token::TableCell(ref vec) => {
                let tag = if this.as_ref().table_head { "th" } else { "td" };
                Ok(format!("<{}>{}</{}>", tag, this.render_vec(vec)?, tag))
            }
            Token::TableHead(ref vec) => {
                this.as_mut().table_head = true;
                let s = this.render_vec(vec)?;
                this.as_mut().table_head = false;
                Ok(format!("<tr>\n{}</tr>\n", s))
            }
            Token::Footnote(ref vec) => {
                this.as_mut().footnote_number += 1;
                let number = this.as_ref().footnote_number;
                assert!(!vec.is_empty());

                let note_number = format!("<p class = \"note-number\">
  <a href = \"#note-source-{}\">[{}]</a>
</p>\n",
                                          number,
                                          number);

                let inner = format!("<aside id = \"note-dest-{}\">{}</aside>",
                                    number,
                                    this.render_vec(vec)?);
                this.as_mut().footnotes.push((note_number, inner));

                Ok(format!("<a href = \"#note-dest-{}\"><sup id = \
                            \"note-source-{}\">[{}]</sup></a>",
                           number,
                           number,
                           number))
            }
            Token::__NonExhaustive => unreachable!(),
        }
    }

    /// Consider the html as a template
    fn templatize(&mut self, s: &str) -> Result<String> {
        let mapbuilder = self.book.get_metadata(|s| Ok(s.to_owned()))?;
        let data = mapbuilder.build();
        let template =
            compile_str(s, &self.book.source, "")?;
        let mut res = vec![];
        template.render_data(&mut res, &data)?;
        Ok(String::from_utf8_lossy(&res).into_owned())
    }

    /// Renders the toc name
    #[doc(hidden)]
    pub fn get_toc_name(&mut self) -> Result<String> {
        let data = self.book
            .get_metadata(|s| self.render_vec(&Parser::new().parse_inline(s)?))?
            .build();
        let template = self.book.options.get_str("rendering.inline_toc.name").unwrap();
        let template = compile_str(template,
                                   &self.book.source,
                                   "rendering.inline_toc.name")?;
        let mut res = vec![];
        template.render_data(&mut res, &data)?;
        Ok(String::from_utf8_lossy(&res).into_owned())
    }


    /// Renders a footer, which can include a "Generated by Crowboook" link
    /// or a customized text
    #[doc(hidden)]
    pub fn get_footer<T>(this: &mut T) -> Result<String>
        where T: AsMut<HtmlRenderer<'a>> + AsRef<HtmlRenderer<'a>> + Renderer
    {
        let content = if let Ok(footer) = this.as_ref().book.options.get_str("html.footer") {
            match this.as_mut().templatize(footer) {
                Ok(content) => content,
                Err(err) => {
                    return Err(Error::render(&this.as_ref().book.source,
                                             lformat!("rendering 'html.footer' \
                                                       template:\n{error}",
                                                      error = err)))
                }
            }

        } else {
            String::new()
        };
        if content.is_empty() {
            Ok(content)
        } else {
            let tokens = Parser::new().parse(&content)?;
            let content = this.render_vec(&tokens)?;
            Ok(format!("<footer id = \"footer\">{}</footer>", content))
        }
    }

    /// Renders a header
    #[doc(hidden)]
    pub fn get_header<T>(this: &mut T) -> Result<String>
        where T: AsMut<HtmlRenderer<'a>> + AsRef<HtmlRenderer<'a>> + Renderer
    {
        if let Ok(top) = this.as_ref().book.options.get_str("html.header") {
            match this.as_mut().templatize(top) {
                Ok(content) => {
                    Ok(format!("<div id = \"top\">{}</div>",
                               this.render_vec(&Parser::new().parse(&content)?)?))
                }
                Err(err) => {
                    Err(Error::render(&this.as_ref().book.source,
                                      (lformat!("rendering 'html.header' template:\n{error}",
                                                error = err))))
                }
            }
        } else {
            Ok(String::new())
        }
    }
}

impl<'a> AsMut<HtmlRenderer<'a>> for HtmlRenderer<'a> {
    fn as_mut(&mut self) -> &mut HtmlRenderer<'a> {
        self
    }
}

impl<'a> AsRef<HtmlRenderer<'a>> for HtmlRenderer<'a> {
    fn as_ref(&self) -> &HtmlRenderer<'a> {
        self
    }
}

impl<'a> Renderer for HtmlRenderer<'a> {
    fn render_token(&mut self, token: &Token) -> Result<String> {
        HtmlRenderer::static_render_token(self, token)
    }
}


/// This macro automatically generates AsRef and AsMut implementations
/// for a type, to itself and to HtmlRenderer. Type must have a .html element
/// and use a <'a> lifetime parameter.
///
/// # Example
///
/// ```
/// #[macro_use]
/// extern crate crowbook;
/// use crowbook::{HtmlRenderer, Renderer, Token, Result};
/// struct Foo<'a> {
///     html: HtmlRenderer<'a>,
/// }
///
/// derive_html!{Foo<'a>, HtmlRenderer::static_render_token}
/// fn main() {}
/// ```
macro_rules! derive_html {
    {$t:ty, $f:path} => (
        impl<'a> AsRef<HtmlRenderer<'a>> for $t {
            fn as_ref(&self) -> &HtmlRenderer<'a> {
                &self.html
            }
        }

        impl<'a> AsMut<HtmlRenderer<'a>> for $t {
            fn as_mut(&mut self) -> &mut HtmlRenderer<'a> {
                &mut self.html
            }
        }

        impl<'a> AsRef<$t> for $t {
            fn as_ref(&self) -> &$t {
                self
            }
        }

        impl<'a> AsMut<$t> for $t {
            fn as_mut(&mut self) -> &mut $t {
                self
            }
        }

        impl<'a> Renderer for $t {
            fn render_token(&mut self, token: &Token) -> Result<String> {
                $f(self, token)
            }
        }

    );
}
