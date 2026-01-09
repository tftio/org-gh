use crate::error::Result;
use crate::github::model::{
    CreateIssueRequest, GhComment, GhIssue, GhIssueState, UpdateIssueRequest,
};

pub struct GitHubClient {
    client: octocrab::Octocrab,
    owner: String,
    repo: String,
}

impl GitHubClient {
    pub async fn new(token: &str, repo: &str) -> Result<Self> {
        Self::with_base_url(token, repo, None).await
    }

    /// Create a client with an optional base URL (for testing with wiremock)
    pub async fn with_base_url(token: &str, repo: &str, base_url: Option<&str>) -> Result<Self> {
        let (owner, repo_name) = parse_repo(repo)?;

        let mut builder = octocrab::Octocrab::builder().personal_token(token.to_string());

        if let Some(url) = base_url {
            builder = builder.base_uri(url)?;
        }

        let client = builder.build()?;

        Ok(Self {
            client,
            owner,
            repo: repo_name,
        })
    }

    /// Fetch all open and recently closed issues
    pub async fn fetch_issues(&self) -> Result<Vec<GhIssue>> {
        let mut all_issues = Vec::new();

        // Fetch open issues
        let open_issues = self
            .client
            .issues(&self.owner, &self.repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await?;

        for issue in open_issues.items {
            all_issues.push(convert_issue(issue));
        }

        // Fetch closed issues (recent)
        let closed_issues = self
            .client
            .issues(&self.owner, &self.repo)
            .list()
            .state(octocrab::params::State::Closed)
            .per_page(100)
            .send()
            .await?;

        for issue in closed_issues.items {
            all_issues.push(convert_issue(issue));
        }

        Ok(all_issues)
    }

    /// Fetch a single issue by number
    pub async fn get_issue(&self, number: u64) -> Result<GhIssue> {
        let issue = self
            .client
            .issues(&self.owner, &self.repo)
            .get(number)
            .await?;

        Ok(convert_issue(issue))
    }

    /// Create a new issue
    pub async fn create_issue(&self, req: CreateIssueRequest) -> Result<GhIssue> {
        let issues = self.client.issues(&self.owner, &self.repo);
        let mut builder = issues.create(&req.title);

        if let Some(ref body) = req.body {
            builder = builder.body(body);
        }

        if !req.assignees.is_empty() {
            builder = builder.assignees(req.assignees.clone());
        }

        if !req.labels.is_empty() {
            builder = builder.labels(req.labels.clone());
        }

        let issue = builder.send().await?;
        Ok(convert_issue(issue))
    }

    /// Update an existing issue
    pub async fn update_issue(&self, number: u64, req: UpdateIssueRequest) -> Result<GhIssue> {
        let issues = self.client.issues(&self.owner, &self.repo);
        let mut builder = issues.update(number);

        if let Some(ref title) = req.title {
            builder = builder.title(title);
        }

        if let Some(ref body) = req.body {
            builder = builder.body(body);
        }

        if let Some(state) = req.state {
            builder = builder.state(match state {
                GhIssueState::Open => octocrab::models::IssueState::Open,
                GhIssueState::Closed => octocrab::models::IssueState::Closed,
            });
        }

        if let Some(ref assignees) = req.assignees {
            builder = builder.assignees(assignees);
        }

        if let Some(ref labels) = req.labels {
            builder = builder.labels(labels);
        }

        let issue = builder.send().await?;
        Ok(convert_issue(issue))
    }

    /// Close an issue
    pub async fn close_issue(&self, number: u64) -> Result<GhIssue> {
        self.update_issue(
            number,
            UpdateIssueRequest {
                state: Some(GhIssueState::Closed),
                ..Default::default()
            },
        )
        .await
    }

    /// Reopen an issue
    pub async fn reopen_issue(&self, number: u64) -> Result<GhIssue> {
        self.update_issue(
            number,
            UpdateIssueRequest {
                state: Some(GhIssueState::Open),
                ..Default::default()
            },
        )
        .await
    }

    /// Fetch comments for an issue
    pub async fn fetch_comments(&self, issue_number: u64) -> Result<Vec<GhComment>> {
        let comments = self
            .client
            .issues(&self.owner, &self.repo)
            .list_comments(issue_number)
            .per_page(100)
            .send()
            .await?;

        Ok(comments
            .items
            .into_iter()
            .map(|c| GhComment {
                id: c.id.0,
                author: c.user.login,
                body: c.body.unwrap_or_default(),
                created_at: c.created_at,
                updated_at: c.updated_at.unwrap_or(c.created_at),
            })
            .collect())
    }

    /// Try to find an existing issue by title (for initial matching)
    pub async fn find_by_title(&self, title: &str) -> Result<Option<GhIssue>> {
        let issues = self.fetch_issues().await?;
        Ok(issues.into_iter().find(|i| i.title == title))
    }
}

fn parse_repo(repo: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err(crate::error::Error::Config(format!(
            "Invalid repository format: {}. Expected owner/repo",
            repo
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn convert_issue(issue: octocrab::models::issues::Issue) -> GhIssue {
    GhIssue {
        number: issue.number,
        title: issue.title,
        body: issue.body,
        state: match issue.state {
            octocrab::models::IssueState::Open => GhIssueState::Open,
            octocrab::models::IssueState::Closed => GhIssueState::Closed,
            _ => GhIssueState::Open,
        },
        assignees: issue.assignees.into_iter().map(|a| a.login).collect(),
        labels: issue.labels.into_iter().map(|l| l.name).collect(),
        created_at: issue.created_at,
        updated_at: issue.updated_at,
        closed_at: issue.closed_at,
        html_url: issue.html_url.to_string(),
    }
}
