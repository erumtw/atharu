//! Names list (Settings › Transcription): how people's names are said → who they are. Fed to the speech model as
//! hints so a nickname comes out spelled right, and to /meeting (via `atharu meeting`) to fix the rest and pick
//! owners. One line per person: `แฟรง = Frank, VP Strategy`; the part after `=` is optional, `#` starts a comment.

pub struct Person {
    /// As said in meetings (the speech model's spelling target).
    pub said: String,
    /// Who that is, free text; empty when only the spelling matters.
    pub who: String,
}

/// A long list would crowd out the rest of the speech model's prompt; past this it's ignored.
pub const MAX: usize = 40;

pub fn parse(text: &str) -> Vec<Person> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (said, who) = l.split_once('=').unwrap_or((l, ""));
            let said = said.trim();
            (!said.is_empty()).then(|| Person {
                said: said.to_string(),
                who: who.trim().to_string(),
            })
        })
        .take(MAX)
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn lines_comments_and_bare_names() {
        let p = super::parse("แฟรง = Frank, VP Strategy\n# team\n\nออฟ=Off (PM)\nไอติม\n = nobody");
        let got: Vec<(&str, &str)> = p
            .iter()
            .map(|x| (x.said.as_str(), x.who.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                ("แฟรง", "Frank, VP Strategy"),
                ("ออฟ", "Off (PM)"),
                ("ไอติม", "")
            ]
        );
    }
}
