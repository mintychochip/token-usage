//! Publish a usage bundle to a GitHub Pages repository.

use std::path::Path;
use std::process::Command;

use crate::publish::{gh_bin, run_gh, PublishBundle};
use crate::website_embed_html;

fn split_repo(repo: &str) -> Result<(&str, &str), String> {
    let mut parts = repo.split('/');
    let owner = parts.next().ok_or("repo must be owner/name")?;
    let name = parts.next().ok_or("repo must be owner/name")?;
    if parts.next().is_some() || owner.is_empty() || name.is_empty() {
        return Err("repo must be owner/name".into());
    }
    Ok((owner, name))
}

fn run_git(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("failed to run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git failed: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_commit(dir: &Path, message: &str) -> Result<(), String> {
    let path = dir
        .to_str()
        .ok_or_else(|| "invalid publish directory".to_string())?;

    let mut add = Command::new("git");
    add.args(["-C", path, "add", "."]);
    run_git(&mut add)?;

    let mut commit = Command::new("git");
    commit.args([
        "-C",
        path,
        "-c",
        "user.name=token-usage",
        "-c",
        "user.email=noreply@localhost",
        "commit",
        "-m",
        message,
    ]);

    let out = commit.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if !stderr.contains("nothing to commit") {
            return Err(format!("git commit failed: {stderr}"));
        }
    }
    Ok(())
}

fn write_bundle_files(dir: &Path, bundle: &PublishBundle, card_js: &str) -> Result<(), String> {
    std::fs::write(dir.join("usage-summary.json"), &bundle.summary_json)
        .map_err(|e| e.to_string())?;
    std::fs::write(dir.join("usage-badge.json"), &bundle.shields_json)
        .map_err(|e| e.to_string())?;
    std::fs::write(dir.join("token-usage-card.js"), card_js).map_err(|e| e.to_string())?;

    let summary_url = "usage-summary.json";
    let card = website_embed_html(summary_url);
    std::fs::write(
        dir.join("index.html"),
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"/><title>token usage</title></head><body>{}</body></html>\n",
            card
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn gh_repo_exists(repo: &str) -> Result<bool, String> {
    let mut cmd = Command::new(gh_bin());
    cmd.args(["repo", "view", repo]);
    let out = cmd.output().map_err(|e| format!("failed to run gh repo view: {e}"))?;
    if out.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    if stderr.contains("not found") || stderr.contains("could not resolve") {
        Ok(false)
    } else {
        Err(format!("gh repo view failed: {}", String::from_utf8_lossy(&out.stderr)))
    }
}

/// Publish `bundle` to `repo`, creating the repo if it does not exist.
///
/// Returns the GitHub Pages URL, e.g. `https://owner.github.io/repo`.
pub fn publish(
    repo: &str,
    bundle: &PublishBundle,
    card_js: &str,
    generated_at: u64,
) -> Result<String, String> {
    let (owner, name) = split_repo(repo)?;

    let work = std::env::temp_dir().join(format!(
        "token-usage-ghpages-{}-{}",
        std::process::id(),
        generated_at
    ));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;

    if !gh_repo_exists(repo)? {
        // Create a fresh git repo, populate it, and push it to a new GitHub repo.
        let path = work
            .to_str()
            .ok_or_else(|| "invalid publish directory".to_string())?;
        let mut init = Command::new("git");
        init.args(["-C", path, "init"]);
        run_git(&mut init)?;

        write_bundle_files(&work, bundle, card_js)?;
        git_commit(&work, "initial token usage")?;

        let mut create = Command::new(gh_bin());
        create
            .args(["repo", "create", repo, "--public", "--source=.", "--push"])
            .current_dir(&work);
        run_gh(&mut create)?;
    } else {
        // Clone the existing repo, overwrite the files, commit, and push.
        let clone = work.join("clone");
        let mut clone_cmd = Command::new(gh_bin());
        clone_cmd.args([
            "repo",
            "clone",
            repo,
            clone
                .to_str()
                .ok_or_else(|| "invalid clone directory".to_string())?,
        ]);
        run_gh(&mut clone_cmd)?;

        write_bundle_files(&clone, bundle, card_js)?;
        git_commit(&clone, "update token usage")?;

        let mut push = Command::new("git");
        push.args([
            "-C",
            clone
                .to_str()
                .ok_or_else(|| "invalid clone directory".to_string())?,
            "push",
        ]);
        run_git(&mut push)?;
    }

    Ok(format!("https://{owner}.github.io/{name}"))
}
