//! Asks GitHub which releases exist, for the header's update badge and for the notice
//! that a later major version is out.
//!
//! An update is a newer release of the running major version. A later major version is
//! reported apart from that: it has breaking changes, and the usual update commands do
//! not install it.

use crate::state::{MajorRelease, UpdateStatus};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use std::time::Duration;

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// How many of the newest releases to read. Enough to hold the newest release of every
/// major version that still gets releases.
const RELEASES_PER_PAGE: u8 = 30;

/// Longest release title shown to the user.
const MAX_TITLE_CHARS: usize = 60;

/// Environment variable that turns the check off when set to anything but `0` or `false`.
pub const DISABLE_ENV: &str = "RALPH_NO_UPDATE_CHECK";

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// What the release list says about the running version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCheck {
    /// Whether a newer release of the running major version exists.
    pub status: UpdateStatus,
    /// The newest release of a later major version, if there is one.
    pub new_major: Option<MajorRelease>,
}

/// True when the user or the environment has turned the check off.
///
/// `RALPH_NO_UPDATE_CHECK` turns it off, and so does `CI`: nobody reads a notice there.
pub fn update_check_disabled() -> bool {
    disabled_by(|name| std::env::var(name).ok())
}

fn disabled_by(var: impl Fn(&str) -> Option<String>) -> bool {
    let set = |name: &str| {
        var(name).is_some_and(|value| {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        })
    };
    set(DISABLE_ENV) || set("CI")
}

/// Reads the release list. `None` when the check is turned off, GitHub did not answer,
/// or the running version cannot be compared.
pub async fn fetch_update_check() -> Option<UpdateCheck> {
    if update_check_disabled() {
        return None;
    }
    let releases = fetch_releases().await.ok()?;
    check_releases(env!("CARGO_PKG_VERSION"), &releases, &repository())
}

/// The update badge's status alone.
pub async fn fetch_update_status() -> UpdateStatus {
    fetch_update_check()
        .await
        .map_or(UpdateStatus::Unknown, |check| check.status)
}

async fn fetch_releases() -> Result<Vec<Release>, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(concat!("ralph-tui/", env!("CARGO_PKG_VERSION"))),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(UPDATE_CHECK_TIMEOUT)
        .build()?;

    client
        .get(releases_api_url())
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<Release>>()
        .await
}

/// The `owner/name` of the repository whose releases are read.
fn repository() -> String {
    let repository = env!("CARGO_PKG_REPOSITORY").trim_end_matches('/');
    repository
        .strip_prefix("https://github.com/")
        .or_else(|| repository.strip_prefix("http://github.com/"))
        .map(|value| value.trim_end_matches(".git"))
        .filter(|value| !value.is_empty())
        .unwrap_or("mikeyobrien/ralph-orchestrator")
        .to_string()
}

fn releases_api_url() -> String {
    format!(
        "https://api.github.com/repos/{}/releases?per_page={RELEASES_PER_PAGE}",
        repository()
    )
}

/// Sorts the published releases into an update of the running major version and the
/// newest release of a later major version. Drafts, prereleases and tags that are not
/// versions are skipped.
fn check_releases(current: &str, releases: &[Release], repository: &str) -> Option<UpdateCheck> {
    let running = parse_version(current)?;
    let major = *running.first()?;

    let mut newest_here: Option<Vec<u64>> = None;
    let mut newest_major: Option<(Vec<u64>, &Release)> = None;
    for release in releases
        .iter()
        .filter(|release| !release.prerelease && !release.draft)
    {
        let Some(version) = parse_version(&release.tag_name) else {
            continue;
        };
        let Some(&release_major) = version.first() else {
            continue;
        };
        if release_major == major {
            if newest_here.as_ref().is_none_or(|newest| version > *newest) {
                newest_here = Some(version);
            }
        } else if release_major > major
            && newest_major
                .as_ref()
                .is_none_or(|(newest, _)| version > *newest)
        {
            newest_major = Some((version, release));
        }
    }

    let status = match newest_here {
        Some(newest) if newest > running => UpdateStatus::Available {
            latest: display_version(&newest),
        },
        Some(_) => UpdateStatus::UpToDate,
        None => UpdateStatus::Unknown,
    };
    let new_major = newest_major.map(|(version, release)| {
        let version = display_version(&version);
        MajorRelease {
            title: release_title(release.name.as_deref(), &version),
            url: release_url(&release.html_url, repository),
            version,
        }
    });

    Some(UpdateCheck { status, new_major })
}

/// The release's own title, cleaned for a terminal, or the version when it has none.
fn release_title(name: Option<&str>, version: &str) -> String {
    let title: String = name
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_TITLE_CHARS)
        .collect();
    let title = title.trim();
    if title.is_empty() {
        format!("Version {version}")
    } else {
        title.to_string()
    }
}

/// The release's page. Anything that is not a plain GitHub address is replaced with the
/// repository's release list, so a notice never prints an address from elsewhere.
fn release_url(html_url: &str, repository: &str) -> String {
    let plain = html_url.starts_with("https://github.com/")
        && html_url
            .chars()
            .all(|character| character.is_ascii_graphic());
    if plain {
        html_url.to_string()
    } else {
        format!("https://github.com/{repository}/releases")
    }
}

fn normalize_version(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_start_matches('v');
    let core = trimmed.split(['-', '+']).next()?.trim();
    if core.is_empty() || !core.split('.').all(|part| !part.is_empty()) {
        return None;
    }
    Some(core.to_string())
}

fn parse_version(version: &str) -> Option<Vec<u64>> {
    normalize_version(version)?
        .split('.')
        .map(|part| part.parse().ok())
        .collect()
}

fn display_version(version: &[u64]) -> String {
    version
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPOSITORY: &str = "example/ralph";

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.to_string(),
            name: None,
            html_url: format!("https://github.com/{REPOSITORY}/releases/tag/{tag}"),
            prerelease: false,
            draft: false,
        }
    }

    fn check(current: &str, releases: &[Release]) -> UpdateCheck {
        check_releases(current, releases, REPOSITORY).expect("comparable version")
    }

    #[test]
    fn normalizes_release_tags() {
        assert_eq!(normalize_version("v2.8.0"), Some("2.8.0".to_string()));
        assert_eq!(normalize_version("2.8.0-beta.1"), Some("2.8.0".to_string()));
        assert_eq!(
            normalize_version("2.8.0+build.5"),
            Some("2.8.0".to_string())
        );
    }

    #[test]
    fn rejects_invalid_versions() {
        assert_eq!(normalize_version(""), None);
        assert_eq!(normalize_version("v"), None);
        assert_eq!(normalize_version("2..8"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(parse_version("2.7.0") < parse_version("2.8.0"));
        assert_eq!(parse_version("2.8.0"), parse_version("v2.8.0"));
        assert!(parse_version("2.10.0") > parse_version("2.9.9"));
    }

    #[test]
    fn builds_github_releases_api_url() {
        assert_eq!(
            releases_api_url(),
            "https://api.github.com/repos/mikeyobrien/ralph-orchestrator/releases?per_page=30"
        );
    }

    #[test]
    fn offers_the_newest_release_of_the_running_major() {
        let found = check("2.11.0", &[release("v2.11.1"), release("v2.12.0")]);
        assert_eq!(
            found.status,
            UpdateStatus::Available {
                latest: "2.12.0".to_string()
            }
        );
        assert_eq!(found.new_major, None);
    }

    #[test]
    fn a_later_major_is_not_an_update() {
        let found = check("2.12.0", &[release("v3.0.0"), release("v2.12.0")]);
        assert_eq!(found.status, UpdateStatus::UpToDate);
        assert_eq!(
            found.new_major,
            Some(MajorRelease {
                version: "3.0.0".to_string(),
                title: "Version 3.0.0".to_string(),
                url: "https://github.com/example/ralph/releases/tag/v3.0.0".to_string(),
            })
        );
    }

    #[test]
    fn reports_an_update_and_a_later_major_together() {
        let found = check(
            "2.11.0",
            &[
                release("v3.1.0"),
                release("v2.12.1"),
                release("v3.0.0"),
                release("v2.12.0"),
                release("v4.0.0"),
            ],
        );
        assert_eq!(
            found.status,
            UpdateStatus::Available {
                latest: "2.12.1".to_string()
            }
        );
        assert_eq!(
            found.new_major.map(|major| major.version),
            Some("4.0.0".to_string())
        );
    }

    #[test]
    fn skips_drafts_prereleases_and_tags_that_are_not_versions() {
        let mut draft = release("v3.0.0");
        draft.draft = true;
        let mut prerelease = release("v3.0.0-rc.1");
        prerelease.prerelease = true;
        let found = check(
            "2.11.0",
            &[draft, prerelease, release("nightly"), release("v2.11.0")],
        );
        assert_eq!(found.status, UpdateStatus::UpToDate);
        assert_eq!(found.new_major, None);
    }

    #[test]
    fn status_is_unknown_when_the_running_major_has_no_release_listed() {
        let found = check("2.11.0", &[release("v3.0.0")]);
        assert_eq!(found.status, UpdateStatus::Unknown);
        assert!(found.new_major.is_some());
    }

    #[test]
    fn an_incomparable_running_version_gives_no_result() {
        assert_eq!(
            check_releases("dev", &[release("v3.0.0")], REPOSITORY),
            None
        );
    }

    #[test]
    fn uses_the_release_title_without_control_characters() {
        let mut titled = release("v3.0.0");
        titled.name = Some("Ralph\u{1b}[31m 3.0.0\n".to_string());
        let found = check("2.12.0", &[titled]);
        assert_eq!(
            found.new_major.map(|major| major.title),
            Some("Ralph[31m 3.0.0".to_string())
        );
    }

    #[test]
    fn replaces_an_address_that_is_not_a_plain_github_address() {
        let mut elsewhere = release("v3.0.0");
        elsewhere.html_url = "https://example.test/releases/v3.0.0".to_string();
        let found = check("2.12.0", &[elsewhere]);
        assert_eq!(
            found.new_major.map(|major| major.url),
            Some("https://github.com/example/ralph/releases".to_string())
        );
    }

    #[test]
    fn the_check_is_off_when_asked_or_in_ci() {
        let with = |name: &'static str, value: &'static str| {
            move |asked: &str| (asked == name).then(|| value.to_string())
        };
        assert!(!disabled_by(|_| None));
        assert!(disabled_by(with(DISABLE_ENV, "1")));
        assert!(disabled_by(with("CI", "true")));
        assert!(!disabled_by(with(DISABLE_ENV, "0")));
        assert!(!disabled_by(with(DISABLE_ENV, "false")));
        assert!(!disabled_by(with(DISABLE_ENV, "")));
    }
}
