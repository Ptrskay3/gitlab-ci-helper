use gitlab::api::{
    projects::{merge_requests::CreateMergeRequest, repository},
    Query,
};
use serde::Deserialize;
use tracing::Level;
use tracing_subscriber::{
    fmt::writer::MakeWriterExt, layer::SubscriberExt, util::SubscriberInitExt,
};
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

#[derive(Debug, Deserialize)]
struct Branch {
    name: String,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr.with_max_level(Level::INFO))
                .without_time()
                .with_target(false),
        )
        .init();
    dotenvy::dotenv().ok();
    let client = if std::env::var("CI").is_ok() {
        tracing::info!("hello");
        gitlab::Gitlab::new_job_token("gitlab.zengo.eu", std::env::var("CI_JOB_TOKEN")?)?
    } else {
        gitlab::Gitlab::new("gitlab.zengo.eu", std::env::var("ACCESS_TOKEN")?)?
    };

    let branches = repository::branches::Branches::builder()
        .project(GITLAB_PROJECT_ID)
        .regex(r"release/\d+\.\d+\.\d+")
        .build()?;
    let branches: Vec<Branch> = branches.query(&client)?;
    let Some(latest_release) = branches
        .iter()
        .map(|branch| semver::Version::parse(branch.name.split('/').last().unwrap()).unwrap())
        .max()
    else {
        anyhow::bail!("No branches found based on the release/x.x.x pattern")
    };
    let emergency_patch = semver::Version::new(
        latest_release.major,
        latest_release.minor,
        latest_release.patch + 1,
    );

    let latest_release = format!("release/{latest_release}");
    let emergency_patch = format!("release/{}", emergency_patch);
    tracing::info!(
        latest_release,
        emergency_patch,
        "creating a new patch from latest release..."
    );
    let create_branch = repository::branches::CreateBranch::builder()
        .project(GITLAB_PROJECT_ID)
        .branch(&emergency_patch)
        .ref_(&latest_release)
        .build()?;
    let _: Result<serde_json::Value, _> = create_branch.query(&client);

    let mr = CreateMergeRequest::builder()
        .project(GITLAB_PROJECT_ID)
        .source_branch(&emergency_patch)
        .target_branch("master")
        .title(format!("EMERGENCY PRODUCTION PATCH ({})", latest_release))
        .description(format!(
            "## This is an auto-generated emergency patch aimed at PRODUCTION. 

To start working, switch to this branch:
```bash
git pull origin {emergency_patch} && git checkout {emergency_patch}
```

Please fill out the following checklist:

### Why this change is necessary?

### What does this change do?

### How to test this change?"
        ))
        .assignee(std::env::var("GITLAB_USER_ID")?.parse()?)
        .build()?;

    let _: Result<serde_json::Value, _> = mr.query(&client);

    let mr2 = CreateMergeRequest::builder()
        .project(GITLAB_PROJECT_ID)
        .source_branch(&emergency_patch)
        .target_branch("dev")
        .title(format!("EMERGENCY PRODUCTION PATCH ({})", latest_release))
        .description(format!(
            "## This is an auto-generated emergency patch aimed at PRODUCTION. 

To start working, switch to this branch:
```bash
git pull origin {emergency_patch} && git checkout {emergency_patch}
```

Please fill out the following checklist:

### Why this change is necessary?

### What does this change do?

### How to test this change?"
        ))
        .assignee(std::env::var("GITLAB_USER_ID")?.parse()?)
        .build()?;

    let _: Result<serde_json::Value, _> = mr2.query(&client);

    Ok(())
}
