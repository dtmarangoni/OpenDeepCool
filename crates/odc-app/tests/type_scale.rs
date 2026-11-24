//! One size for each level of the text (Phase 11a Task 55; owner, 2026-11-23): a category (a
//! card's heading, a tile group's) is 1.2em wherever it is, what is directly in a category is
//! 1em, and what is in that 0.83em, so a child is never larger than the heading that holds it.
//! Read from the stylesheet the app loads and from the mockups' own, so the two agree.

use std::fs;
use std::path::Path;

/// The declarations of the first rule of `css` whose selector list has `selector` as one of its
/// comma-separated selectors, comments removed.
fn rule<'a>(css: &'a str, selector: &str) -> &'a str {
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}').expect("a closed rule") + open;
        let (head, body) = (&rest[..open], &rest[open + 1..close]);
        // The selector list is what follows the previous rule's `}` or comment's `*/`.
        let list = head.rsplit(['}', '/']).next().unwrap_or(head);
        if list.split(',').any(|s| s.trim() == selector) {
            return body;
        }
        rest = &rest[close + 1..];
    }
    panic!("no rule for {selector}");
}

/// The body of the first rule for `selector` that declares `property`: a style sheet may have
/// several rules for one selector (the mockups' `:root` holds the palettes and the sizes apart).
fn rule_declaring<'a>(css: &'a str, selector: &str, property: &str) -> &'a str {
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let close = rest[open..].find('}').expect("a closed rule") + open;
        let (head, body) = (&rest[..open], &rest[open + 1..close]);
        let list = head.rsplit(['}', '/']).next().unwrap_or(head);
        if list.split(',').any(|s| s.trim() == selector) && value(body, property).is_some() {
            return body;
        }
        rest = &rest[close + 1..];
    }
    panic!("no rule for {selector} that declares {property}");
}

/// A declaration's value in a rule's body, or `None`.
fn value<'a>(body: &'a str, property: &str) -> Option<&'a str> {
    body.split(';').find_map(|d| {
        let (name, value) = d.split_once(':')?;
        (name.trim() == property).then_some(value.trim())
    })
}

fn app_css() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.css")).expect("app.css")
}

fn size(css: &str, selector: &str) -> Option<String> {
    value(rule(css, selector), "font-size").map(str::to_owned)
}

#[test]
fn the_type_scale_has_one_size_for_each_level() {
    let css = app_css();
    // Level 0, the page's title and the off screen's heading.
    assert_eq!(size(&css, ".page-title").as_deref(), Some("1.44em"));
    assert_eq!(size(&css, ".heading").as_deref(), Some("1.44em"));
    // Level 1, a category, whichever card or page it is on.
    assert_eq!(size(&css, ".card-heading").as_deref(), Some("1.2em"));
    assert_eq!(size(&css, ".group-heading").as_deref(), Some("1.2em"));
    // Level 3, what is two levels in: a hint, and a row of the Default Theme.
    assert_eq!(size(&css, ".small").as_deref(), Some("0.83em"));
    assert_eq!(
        size(&css, ".subcategory .row-title").as_deref(),
        Some("0.83em")
    );
}

#[test]
fn a_subcategory_heading_has_the_size_of_a_row_and_is_not_smaller_than_its_rows() {
    let css = app_css();
    // Level 2 is the body size: the heading of the Default Theme sets none of its own, so it is
    // the same size as a row's title in a card, and larger than the rows it holds (0.83em).
    assert_eq!(size(&css, ".sub-heading"), None);
    assert_eq!(
        value(rule(&css, ".sub-heading"), "font-weight"),
        Some("600")
    );
}

#[test]
fn the_mockups_draw_the_same_levels() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/design/mockups-src/app.css");
    let css = fs::read_to_string(path).expect("the mockups' stylesheet");
    let root = rule_declaring(&css, ":root", "--fs-page-title");
    assert_eq!(
        value(root, "--fs-page-title"),
        Some("calc(var(--fs) * 1.44)")
    );
    assert_eq!(value(root, "--fs-level1"), Some("calc(var(--fs) * 1.2)"));
    assert_eq!(value(root, "--fs-small"), Some("calc(var(--fs) * .83)"));
    assert_eq!(
        value(rule(&css, ".group h3"), "font-size"),
        Some("var(--fs-level1)")
    );
    assert_eq!(
        value(rule(&css, ".sub-heading"), "font-size"),
        Some("var(--fs)")
    );
    assert_eq!(
        value(rule(&css, ".subcat .row-l span"), "font-size"),
        Some("var(--fs-small)")
    );
}
