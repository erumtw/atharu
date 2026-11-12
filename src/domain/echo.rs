//! Speaker echo: on laptop speakers the mic hears the call, so the "me" stream repeats what "others"
//! said, a few seconds (one transcription window, up to ~30 s) apart. Hidden on read, the stored
//! transcript stays as recorded.

/// How far apart (seconds) the two copies of one sentence can land: one 30 s window.
const WINDOW: f64 = 30.0;

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Same sentence, allowing a clipped start/end: one side's words appear in order inside the other's.
/// Under 3 words ("Yes.", "Okay.") is too common to call an echo.
fn same(a: &[String], b: &[String]) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= 3 && long.windows(short.len()).any(|w| w == short)
}

/// Thai has no spaces, so a sentence is one "word" above: compare letters instead. Same when the shorter is
/// most of the longer and inside it, or their letter pairs mostly match (whisper spaces and clips each copy
/// differently). Under 6 letters is too short to judge.
fn same_letters(a: &str, b: &str) -> bool {
    let norm = |s: &str| -> Vec<char> {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let (a, b) = (norm(a), norm(b));
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    if short.len() < 6 {
        return false;
    }
    let inside = long.windows(short.len()).any(|w| w == short.as_slice());
    if inside && short.len() * 10 >= long.len() * 6 {
        return true;
    }
    let pairs =
        |v: &Vec<char>| -> Vec<(char, char)> { v.windows(2).map(|w| (w[0], w[1])).collect() };
    let (pa, mut pb) = (pairs(short), pairs(long));
    let mut hit = 0;
    for p in &pa {
        if let Some(i) = pb.iter().position(|q| q == p) {
            pb.swap_remove(i);
            hit += 1;
        }
    }
    // Dice coefficient over letter pairs.
    hit * 2 * 10 >= (pa.len() + pairs(long).len()) * 7
}

/// A made-up speaker label whisper puts at a segment's start (`เมื่อ:`, `กลับ:`, `Glossary:`): the prompt is a
/// `Glossary: …` line and on Thai it imitates the `word:` shape. One short word, no digits (a time `10:30`) and
/// not a URL scheme. Speech itself never carries a colon.
pub fn strip_label(t: &str) -> &str {
    let t = t.trim_start();
    if let Some((head, rest)) = t.split_once(':') {
        let n = head.chars().count();
        if (1..=12).contains(&n)
            && !head
                .chars()
                .any(|c| c.is_whitespace() || c.is_ascii_digit())
            && !matches!(head, "http" | "https")
            && !rest.trim().is_empty()
        {
            return rest.trim_start();
        }
    }
    t
}

/// Drops each "me" segment that repeats an "others" segment within `WINDOW`. Order is kept.
/// Labels stripped too (`strip_label`), so transcripts recorded before the recorder did it read clean.
pub fn drop_echo(segs: Vec<(f64, String, String)>) -> Vec<(f64, String, String)> {
    let segs: Vec<(f64, String, String)> = segs
        .into_iter()
        .map(|(t, s, x)| (t, s, strip_label(&x).to_string()))
        .collect();
    let others: Vec<(f64, Vec<String>, &str)> = segs
        .iter()
        .filter(|(_, s, _)| s == "others")
        .map(|(t, _, x)| (*t, words(x), x.as_str()))
        .collect();
    let keep: Vec<bool> = segs
        .iter()
        .map(|(t, s, x)| {
            s != "me" || {
                let w = words(x);
                !others.iter().any(|(ot, ow, ox)| {
                    (ot - t).abs() <= WINDOW && (same(&w, ow) || same_letters(x, ox))
                })
            }
        })
        .collect();
    segs.into_iter()
        .zip(keep)
        .filter_map(|(seg, k)| k.then_some(seg))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{drop_echo, strip_label};

    #[test]
    fn thai_echo_and_made_up_labels() {
        // The owner's Thai test call (laptop speakers): labels whisper invented, and the mic's copy.
        let out = drop_echo(vec![
            seg(
                12.0,
                "others",
                "เมื่อ: ใช่ ตอนแรกนะ เพื่อนของหมายสี่คนจะไปกับเราด้วย",
            ),
            seg(12.0, "me", "กลับ:ใช่ตอนแรกนะเพื่อนของหมายสี่คนจะไปกับเราด้วย"),
            seg(0.0, "me", "โอเคครับเดี๋ยวก็จะเป็นการทดสอบครับภาษาไทยต่อเนอะ"),
        ]);
        let texts: Vec<&str> = out.iter().map(|s| s.2.as_str()).collect();
        assert_eq!(
            texts,
            [
                "ใช่ ตอนแรกนะ เพื่อนของหมายสี่คนจะไปกับเราด้วย",
                "โอเคครับเดี๋ยวก็จะเป็นการทดสอบครับภาษาไทยต่อเนอะ"
            ]
        );
        assert_eq!(strip_label("at 10:30 we ship"), "at 10:30 we ship");
        assert_eq!(strip_label("https://x.y"), "https://x.y");
        assert_eq!(strip_label("Note: ship it"), "ship it");
    }

    fn seg(t: f64, s: &str, x: &str) -> (f64, String, String) {
        (t, s.into(), x.into())
    }

    #[test]
    fn mic_copy_of_the_call_is_hidden_own_words_stay() {
        // From the owner's first test call (YouTube on laptop speakers).
        let out = drop_echo(vec![
            seg(0.0, "others", "Our first topic is family."),
            seg(15.2, "me", "Our first topic is family."),
            seg(
                20.1,
                "me",
                "And my question is who do you think that you're the most like?",
            ),
            seg(
                22.1,
                "others",
                "My question is, who do you think that you're the most like?",
            ),
            seg(23.6, "me", "I think I like everyone."),
            seg(35.0, "others", "Yes."),
            seg(36.0, "me", "Yes."),
            seg(80.0, "me", "Our first topic is family."),
        ]);
        let me: Vec<&str> = out
            .iter()
            .filter(|s| s.1 == "me")
            .map(|s| s.2.as_str())
            .collect();
        // Short words stay (too common to judge); a repeat 80 s later is the owner quoting, not echo.
        assert_eq!(
            me,
            [
                "I think I like everyone.",
                "Yes.",
                "Our first topic is family."
            ]
        );
        assert_eq!(out.iter().filter(|s| s.1 == "others").count(), 3);
    }
}
