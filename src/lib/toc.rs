use std::iter;


/// A structure for manipulating Table Of Content
#[derive(Debug)]
pub struct Toc {
    elements: Vec<TocElement>,
    numbered: bool,
}

impl Toc {
    /// Create a new, empty, Toc
    pub fn new() -> Toc {
        Toc {
            elements: vec![],
            numbered: false,
        }
    }

    /// Returns `true` if the toc is empty, `false` else.
    ///
    /// Note that `empty` here means that the the toc has zero *or one*
    /// element, since it's still useless in this case.
    pub fn is_empty(&self) -> bool {
        self.elements.len() <= 1
    }

    /// Sets numbering of the Toc
    ///
    /// Only affects whether the generated HTML code should be <ul> or <ol> (epub)
    pub fn numbered(&mut self, numbered: bool) {
        self.numbered = numbered;
    }

    /// Adds an element
    pub fn add(&mut self, level: i32, url: String, title: String) {
        let element = TocElement::new(level, url, title);
        self.elements.push(element);
    }

    /// Render the Toc in a toc.ncx compatible way, for EPUB.
    pub fn render_epub(&mut self, mut offset: u32) -> String {
        self.parts_hack();
        let mut output = String::new();
        let mut levels = vec![];
        
        for element in self.elements.iter() {
            let mut last_level;
            
            loop {
                last_level = if levels.is_empty() {
                    0
                } else {
                    levels[levels.len() - 1]
                };

                if last_level == element.level {
                    output.push_str("    </navPoint>\n");
                    break;
                } else if element.level >= last_level  {
                    levels.push(element.level);
                    break;
                } else /* if element.level < last_level */ {
                    levels.pop().unwrap();
                    output.push_str("    </navPoint>\n");
                    continue;
                }
            }
            
            output.push_str(&format!("
   <navPoint id = \"navPoint-{id}\">
     <navLabel>
       <text>{title}</text>
     </navLabel>
     <content src = \"{url}\" />
",
                                   id = offset,
                                   title = element.title,
                                   url = element.url));
            offset += 1;
        }
        for _ in levels {
            output.push_str("    </navPoint>\n");
        }
        output
    }

    /// Handle parts (sort of) by adding 1 to all levels if lowest level is zero
    fn parts_hack(&mut self) {
        let mut contains_zero = false;
        for elem in &self.elements {
            if elem.level == 0 {
                contains_zero = true;
                break;
            }
        }
        if contains_zero {
            for mut elem in &mut self.elements {
                elem.level += 1;
            }
        }
    }

    /// Render the Toc in either <ul> or <ol> form (according to Self::numbered
    pub fn render(&mut self) -> String {
        self.parts_hack();
        
        let mut output = String::new();

        let mut x = 0;
        let mut level = 0;
        output.push_str(&self.render_vec(&mut x, &mut level));
        for i in (0..level).rev() {
            output.push_str(&format!("{}</{}>",
                                     iter::repeat(' ').take(i as usize).collect::<String>(),
                                     if self.numbered { "ol" } else { "ul" }));
        }
        output
    }

    fn render_vec(&self, x: &mut usize, level: &mut i32) -> String {
        let orig_level = *level;
        let mut content = String::new();
        while *x < self.elements.len() {
            let elem = &self.elements[*x];

            if elem.level <= orig_level {
                return content;
            }

            *x += 1;

            if elem.level > *level {
                for i in *level..elem.level {
                    content.push_str(&format!("{}{}\n",
                                              iter::repeat(' ')
                                                  .take(i as usize)
                                                  .collect::<String>(),
                                              if self.numbered { "<ol>" } else { "<ul>" }));
                    *level = elem.level;
                }
            } else if elem.level < *level {
                for i in (elem.level..*level).rev() {
                    content.push_str(&format!("{}</{}>\n",
                                              iter::repeat(' ')
                                                  .take(i as usize)
                                                  .collect::<String>(),
                                              if self.numbered { "ol" } else { "ul" }));
                }
                *level = elem.level;
            }
            let spaces: String = iter::repeat(' ').take(elem.level as usize).collect();
            content.push_str(&format!("{}<li><a href = \"{}\">{}</a>\n",
                                      spaces,
                                      elem.url,
                                      elem.title));
            content.push_str(&self.render_vec(x, level));

            for i in (elem.level..*level).rev() {
                content.push_str(&format!("{}</{}>\n",
                                          iter::repeat(' ').take(i as usize).collect::<String>(),
                                          if self.numbered { "ol" } else { "ul" }));
            }
            *level = elem.level;
            content.push_str(&format!("{}</li>\n", spaces));

        }
        content
    }
}


#[derive(Debug)]
struct TocElement {
    level: i32,
    url: String,
    title: String,
}

impl TocElement {
    pub fn new(level: i32, url: String, title: String) -> TocElement {
        TocElement {
            level: level,
            url: url,
            title: title,
        }
    }
}


