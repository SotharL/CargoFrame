use super::*;

const ALL: [Style; 5] = [
    Style::Discord,
    Style::Reddit,
    Style::Slack,
    Style::Jira,
    Style::Html,
];

#[test]
fn text_without_marks_comes_out_as_it_went_in() {
    let text = "Crash on save\nSeen on build 1.4.2, twice.\n\nSteps follow.";
    for style in [Style::Discord, Style::Reddit, Style::Slack, Style::Jira] {
        assert_eq!(render(text, style), text, "{style:?}");
    }
    // Teams gets one paragraph, the lines kept apart as they were
    assert_eq!(
        render(text, Style::Html),
        "<p>Crash on save<br>Seen on build 1.4.2, twice.<br><br>Steps follow.</p>"
    );
}

#[test]
fn each_service_gets_its_own_way_of_writing_a_mark() {
    let text = "**bold** *italic* __under__ `code`";
    assert_eq!(
        render(text, Style::Discord),
        "**bold** *italic* __under__ `code`"
    );
    assert_eq!(
        render(text, Style::Reddit),
        "**bold** *italic* under `code`"
    );
    assert_eq!(render(text, Style::Slack), "*bold* _italic_ under `code`");
    assert_eq!(
        render(text, Style::Jira),
        "*bold* _italic_ +under+ {{code}}"
    );
    assert_eq!(
        render(text, Style::Html),
        "<p><b>bold</b> <i>italic</i> <u>under</u> <code>code</code></p>"
    );
}

#[test]
fn marks_nest() {
    assert_eq!(
        render("**bold and *italic* too**", Style::Slack),
        "*bold and _italic_ too*"
    );
    assert_eq!(
        render("*italic with **bold***", Style::Discord),
        "*italic with **bold***"
    );
    assert_eq!(render("**a** and **b**", Style::Jira), "*a* and *b*");
}

#[test]
fn a_mark_counts_only_at_the_edge_of_a_word() {
    for style in [Style::Discord, Style::Reddit, Style::Slack, Style::Jira] {
        for text in [
            "2*3*4",
            "snake__case__name",
            "a * b * c",
            "5 * 6",
            "file__name.py",
        ] {
            assert_eq!(render(text, style), text, "{style:?}: {text}");
        }
    }
}

#[test]
fn a_mark_that_is_never_closed_stays_as_typed() {
    for style in ALL {
        let out = render("**not closed and *neither", style);
        assert!(out.contains("**not closed"), "{style:?}: {out}");
    }
    assert_eq!(render("a lone ` tick", Style::Jira), "a lone ` tick");
    assert_eq!(render("** spaced **", Style::Slack), "** spaced **");
}

#[test]
fn code_hides_what_it_holds() {
    assert_eq!(
        render("`**not bold**` but **bold**", Style::Slack),
        "`**not bold**` but *bold*"
    );
    assert_eq!(render("`a * b`", Style::Jira), "{{a * b}}");
    assert_eq!(
        render("`<b>`", Style::Html),
        "<p><code>&lt;b&gt;</code></p>"
    );
}

#[test]
fn bullets_and_numbers_are_written_for_each_service() {
    let text = "- one\n* two\n1. first\n2) second";
    assert_eq!(
        render(text, Style::Discord),
        "- one\n- two\n1. first\n2. second"
    );
    assert_eq!(
        render(text, Style::Slack),
        "• one\n• two\n1. first\n2. second"
    );
    assert_eq!(render(text, Style::Jira), "* one\n* two\n# first\n# second");
    assert_eq!(
        render(text, Style::Html),
        "<ul><li>one</li><li>two</li></ul><ol><li>first</li><li>second</li></ol>"
    );
}

#[test]
fn numbers_are_counted_again_from_one_in_each_list() {
    assert_eq!(
        render("3. c\n7. d\ntext\n9. e", Style::Discord),
        "1. c\n2. d\ntext\n1. e"
    );
}

#[test]
fn reddit_needs_an_empty_line_before_a_list_or_a_quote() {
    assert_eq!(
        render("Steps:\n- one\n- two", Style::Reddit),
        "Steps:\n\n- one\n- two"
    );
    assert_eq!(render("> said\nthen", Style::Reddit), "> said\nthen");
    assert_eq!(render("then\n> said", Style::Reddit), "then\n\n> said");
    // Discord does not
    assert_eq!(render("Steps:\n- one", Style::Discord), "Steps:\n- one");
}

#[test]
fn a_quote_is_a_block_on_jira_and_teams() {
    assert_eq!(
        render("> one\n> two\nafter", Style::Jira),
        "{quote}\none\ntwo\n{quote}\nafter"
    );
    assert_eq!(
        render("> one\n> two", Style::Html),
        "<blockquote>one<br>two</blockquote>"
    );
    assert_eq!(render("> one", Style::Slack), "> one");
}

#[test]
fn an_empty_bullet_or_a_dash_without_a_space_is_just_text() {
    assert_eq!(render("-\n-x\n- ", Style::Slack), "-\n-x\n-");
    assert_eq!(render("1.5 litres", Style::Jira), "1.5 litres");
}

#[test]
fn slack_and_teams_escape_what_would_be_markup() {
    assert_eq!(
        render("hi <!channel> & co", Style::Slack),
        "hi &lt;!channel&gt; &amp; co"
    );
    assert_eq!(
        render("<script>x</script> & \"q\"", Style::Html),
        "<p>&lt;script&gt;x&lt;/script&gt; &amp; &quot;q&quot;</p>"
    );
    // inside a mark too
    assert_eq!(render("**a<b**", Style::Slack), "*a&lt;b*");
    // Discord and Jira are not escaped: nothing there reads `<` as markup that matters
    assert_eq!(render("a < b", Style::Discord), "a < b");
}

#[test]
fn it_copes_with_odd_input() {
    for style in ALL {
        assert_eq!(render("", style), "");
        // marks at the very end, a lone marker, and many in a row
        let _ = render("**", style);
        let _ = render("*", style);
        let _ = render("`", style);
        let _ = render("****", style);
        let _ = render("*****a*****", style);
        let _ = render("___ __ _", style);
        let _ = render(&"*a ".repeat(500), style);
    }
}

#[test]
fn trailing_spaces_and_lines_are_dropped() {
    assert_eq!(render("one   \ntwo  ", Style::Discord), "one\ntwo");
    assert_eq!(render("one\n\n", Style::Discord), "one");
}

#[test]
fn a_long_text_full_of_lone_marks_is_read_in_a_moment() {
    let text = "*a `b __c ".repeat(8_000);
    let started = std::time::Instant::now();
    for style in ALL {
        let _ = render(&text, style);
    }
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_mark_further_away_than_anyone_would_bold_is_left_as_typed() {
    let far = format!("**{}**", "x".repeat(3_000));
    assert_eq!(render(&far, Style::Slack), far);
    let near = format!("**{}**", "x".repeat(100));
    assert_eq!(
        render(&near, Style::Slack),
        format!("*{}*", "x".repeat(100))
    );
}
