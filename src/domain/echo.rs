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

/// Drops each "me" segment that repeats an "others" segment within `WINDOW`. Order is kept.
pub fn drop_echo(segs: Vec<(f64, String, String)>) -> Vec<(f64, String, String)> {
    let others: Vec<(f64, Vec<String>)> = segs
        .iter()
        .filter(|(_, s, _)| s == "others")
        .map(|(t, _, x)| (*t, words(x)))
        .collect();
    segs.into_iter()
        .filter(|(t, s, x)| {
            s != "me" || {
                let w = words(x);
                !others
                    .iter()
                    .any(|(ot, ow)| (ot - t).abs() <= WINDOW && same(&w, ow))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::drop_echo;

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
