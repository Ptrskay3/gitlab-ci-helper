use semver::Version;
use winnow::{
    ascii::{space0, Caseless},
    combinator::{alt, delimited, terminated},
    error::{ContextError, ParseError, StrContext, StrContextValue},
    prelude::*,
    token::{literal, take_while},
};

#[derive(Debug, PartialEq)]
pub enum Kind {
    Feature,
    Fix,
}

#[derive(Debug, PartialEq)]
pub struct MergeRequest<'a> {
    kind: Kind,
    jira_id: &'a str,
    title: &'a str,
}

const GITLAB_PROJECT_ID: &str = "823";

const GITLAB_USER_ID: &str = "todo: use it as assignee";

fn is_jira_id(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

pub fn parse_kind(input: &mut &str) -> PResult<Kind> {
    alt((
        literal(Caseless("fix")).map(|_| Kind::Fix),
        literal(Caseless("feat")).map(|_| Kind::Feature),
        literal(Caseless("feature")).map(|_| Kind::Feature),
    ))
    .context(StrContext::Label("kind"))
    .context(StrContext::Expected(StrContextValue::Description(
        "fix or feat",
    )))
    .parse_next(input)
}

pub fn parse_jira_id<'a>(input: &'_ mut &'a str) -> PResult<&'a str> {
    (
        space0,
        delimited(
            literal("("),
            delimited(space0, take_while(1.., is_jira_id), space0),
            literal(")"),
        ),
    )
        .context(StrContext::Label("jira id"))
        .context(StrContext::Expected(StrContextValue::Description(
            "a valid jira id",
        )))
        .map(|(_, jira_id)| jira_id)
        .parse_next(input)
}

pub fn parse_title<'a>(input: &'_ mut &'a str) -> PResult<&'a str> {
    (
        space0,
        literal(':'),
        delimited(space0, take_while(1.., |c: char| c.is_ascii()), space0),
    )
        .context(StrContext::Label("title"))
        .context(StrContext::Expected(StrContextValue::Description(
            "any valid title",
        )))
        .map(|(_, _, title)| title)
        .parse_next(input)
}

pub fn parse_merge_request<'a>(
    input: &'_ mut &'a str,
) -> Result<MergeRequest<'a>, ParseError<&'a str, ContextError>> {
    terminated(
        (parse_kind, parse_jira_id, parse_title).map(|(kind, jira_id, title)| MergeRequest {
            kind,
            jira_id,
            title,
        }),
        space0,
    )
    .parse(input)
}

fn main() {
    let mut input = "feat  (   ABC-123   ) :   Fix a bug";
    match parse_merge_request(&mut input) {
        Ok(merge_request) => {
            println!("{merge_request:?}");
        }
        Err(err) => {
            println!("{err}");
        }
    }
}
