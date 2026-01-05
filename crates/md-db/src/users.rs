use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};

use crate::error::{Error, Result};

/// Filename for the org config file.
pub const ORG_CONFIG_FILENAME: &str = "org.kdl";

/// Legacy filename (deprecated, kept for backward compat).
pub const LEGACY_CONFIG_FILENAME: &str = "users.kdl";

/// Lifecycle status for a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserStatus {
    #[default]
    Active,
    Departed,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Departed => write!(f, "departed"),
        }
    }
}

impl FromStr for UserStatus {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "departed" => Ok(Self::Departed),
            _ => Err(Error::FrontmatterParse(format!(
                "unknown user status: '{s}'"
            ))),
        }
    }
}

/// Lifecycle status for a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TeamStatus {
    #[default]
    Active,
    Deprecated,
}

impl fmt::Display for TeamStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Deprecated => write!(f, "deprecated"),
        }
    }
}

impl FromStr for TeamStatus {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "deprecated" => Ok(Self::Deprecated),
            _ => Err(Error::FrontmatterParse(format!(
                "unknown team status: '{s}'"
            ))),
        }
    }
}

/// Whether a user or team is internal or external.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntityKind {
    #[default]
    Internal,
    External,
    Ai,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal => write!(f, "internal"),
            Self::External => write!(f, "external"),
            Self::Ai => write!(f, "ai"),
        }
    }
}

impl FromStr for EntityKind {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "internal" => Ok(Self::Internal),
            "external" => Ok(Self::External),
            "ai" => Ok(Self::Ai),
            _ => Err(Error::FrontmatterParse(format!(
                "unknown entity kind: '{s}'"
            ))),
        }
    }
}

/// Top-level user/team/org configuration loaded from KDL.
#[derive(Debug, Clone)]
pub struct OrgConfig {
    pub users: HashMap<String, UserDef>,
    pub teams: HashMap<String, TeamDef>,
    pub orgs: HashMap<String, OrgDef>,
    pub jira: Vec<JiraConfig>,
}

/// A user definition with handle and arbitrary extra attributes.
#[derive(Debug, Clone)]
pub struct UserDef {
    pub handle: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub teams: Vec<String>,
    pub org: Option<String>,
    pub status: UserStatus,
    pub kind: EntityKind,
    pub extra: BTreeMap<String, String>,
}

/// A team definition with name, member teams, and arbitrary extra attributes.
#[derive(Debug, Clone)]
pub struct TeamDef {
    pub id: String,
    pub name: Option<String>,
    pub lead: Option<String>,
    pub parent: Option<String>,
    pub teams: Vec<String>,
    pub org: Option<String>,
    pub status: TeamStatus,
    pub kind: EntityKind,
    pub extra: BTreeMap<String, String>,
}

/// An organizational entity (legal entity, business unit, division).
#[derive(Debug, Clone)]
pub struct OrgDef {
    pub id: String,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub primary: bool,
    pub extra: BTreeMap<String, String>,
}

/// External issue tracker integration (e.g. Jira).
#[derive(Debug, Clone)]
pub struct JiraConfig {
    pub prefix: String,
    pub url: String,
}

impl OrgConfig {
    /// Load user/team config from a KDL file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FileNotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Parse user/team config from a KDL string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self> {
        let doc: kdl::KdlDocument = content
            .parse()
            .map_err(|e: kdl::KdlError| Error::FrontmatterParse(format!("user config: {e:#}")))?;

        let mut users = HashMap::new();
        let mut teams = HashMap::new();
        let mut orgs = HashMap::new();
        let mut jira = Vec::new();

        for node in doc.nodes() {
            match node.name().value() {
                "user" => {
                    let user = parse_user_def(node)?;
                    users.insert(user.handle.clone(), user);
                }
                "team" => {
                    let team = parse_team_def(node)?;
                    teams.insert(team.id.clone(), team);
                }
                "org" => {
                    let org = parse_org_def(node)?;
                    orgs.insert(org.id.clone(), org);
                }
                "jira" => {
                    if let Some(prefix) = node.entries().first().and_then(|e| e.value().as_string())
                    {
                        let url = node
                            .children()
                            .iter()
                            .flat_map(|d| d.nodes())
                            .find(|n| n.name().value() == "url")
                            .and_then(|n| n.entries().first())
                            .and_then(|e| e.value().as_string())
                            .unwrap_or_default()
                            .to_string();
                        jira.push(JiraConfig {
                            prefix: prefix.to_string(),
                            url,
                        });
                    }
                }
                _other => {
                    // Skip unknown top-level nodes
                }
            }
        }

        Ok(Self {
            users,
            teams,
            orgs,
            jira,
        })
    }

    /// Check if a `@handle` reference is valid (user, team, or org).
    /// Accepts: `@handle` for users, `@team/name` for teams, `@org/name` for orgs.
    pub fn is_valid_ref(&self, reference: &str) -> bool {
        if let Some(stripped) = reference.strip_prefix('@') {
            if let Some(team_name) = stripped.strip_prefix("team/") {
                self.teams.contains_key(team_name)
            } else if let Some(org_name) = stripped.strip_prefix("org/") {
                self.orgs.contains_key(org_name)
            } else {
                self.users.contains_key(stripped)
            }
        } else {
            false
        }
    }

    /// Check if a reference is a valid user (not team or org).
    pub fn is_valid_user(&self, reference: &str) -> bool {
        if let Some(stripped) = reference.strip_prefix('@') {
            if stripped.starts_with("team/") || stripped.starts_with("org/") {
                false
            } else {
                self.users.contains_key(stripped)
            }
        } else {
            false
        }
    }

    /// Check if a reference is a valid org.
    /// Accepts `@org/name` format. Returns false for non-org refs.
    pub fn is_valid_org(&self, reference: &str) -> bool {
        reference
            .strip_prefix("@org/")
            .is_some_and(|name| self.orgs.contains_key(name))
    }

    /// Get all user handles as `@handle`.
    pub fn all_user_handles(&self) -> Vec<String> {
        self.users.keys().map(|h| format!("@{h}")).collect()
    }

    /// Get all team names as `@team/name`.
    pub fn all_team_names(&self) -> Vec<String> {
        self.teams.keys().map(|t| format!("@team/{t}")).collect()
    }

    /// Get all org names as `@org/name`.
    pub fn all_org_names(&self) -> Vec<String> {
        self.orgs.keys().map(|o| format!("@org/{o}")).collect()
    }

    /// Recursively expand all members of a team (users + nested team members).
    /// Returns user handles (without @).
    pub fn expand_team_members(&self, team_id: &str) -> HashSet<String> {
        let mut members = HashSet::new();
        let mut visited = HashSet::new();
        self.expand_team_recursive(team_id, &mut members, &mut visited);
        members
    }

    fn expand_team_recursive(
        &self,
        team_id: &str,
        members: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(team_id.to_string()) {
            return; // prevent cycles
        }

        // Add direct user members
        for (handle, user) in &self.users {
            if user.teams.contains(&team_id.to_string()) {
                members.insert(handle.clone());
            }
        }

        // Recurse into sub-teams
        if let Some(team) = self.teams.get(team_id) {
            for sub_team in &team.teams {
                self.expand_team_recursive(sub_team, members, visited);
            }
        }
    }

    /// Expand an org to include itself and all descendant org ids.
    /// Uses cycle protection.
    pub fn expand_org(&self, org_id: &str) -> HashSet<String> {
        let mut result = HashSet::new();
        result.insert(org_id.to_string());
        // Find orgs whose parent chain leads to org_id
        for id in self.orgs.keys() {
            if id != org_id && self.org_descends_from(id, org_id, &mut HashSet::new()) {
                result.insert(id.clone());
            }
        }
        result
    }

    /// Check if `child` descends from `ancestor` through parent chain.
    fn org_descends_from(
        &self,
        child: &str,
        ancestor: &str,
        visited: &mut HashSet<String>,
    ) -> bool {
        if !visited.insert(child.to_string()) {
            return false; // cycle
        }
        if let Some(org) = self.orgs.get(child) {
            if let Some(ref parent) = org.parent {
                if parent == ancestor {
                    return true;
                }
                return self.org_descends_from(parent, ancestor, visited);
            }
        }
        false
    }

    /// Get the effective org for a user.
    /// Returns user.org if set, else first team's org from user.teams list.
    pub fn user_effective_org(&self, handle: &str) -> Option<String> {
        let user = self.users.get(handle)?;
        if let Some(ref org) = user.org {
            return Some(org.clone());
        }
        // Fall back to first team's org
        for team_id in &user.teams {
            if let Some(team) = self.teams.get(team_id) {
                if let Some(ref org) = team.org {
                    return Some(org.clone());
                }
            }
        }
        None
    }

    /// All users whose effective org is in the expanded org set (including child orgs).
    pub fn users_in_org(&self, org_id: &str) -> HashSet<String> {
        let org_set = self.expand_org(org_id);
        let mut result = HashSet::new();
        for handle in self.users.keys() {
            if let Some(ref eff) = self.user_effective_org(handle) {
                if org_set.contains(eff) {
                    result.insert(handle.clone());
                }
            }
        }
        result
    }

    /// Check if a `@handle` reference points to a departed user.
    /// Returns false for teams, orgs, and unknown handles.
    pub fn is_departed_user(&self, reference: &str) -> bool {
        if let Some(stripped) = reference.strip_prefix('@') {
            if stripped.starts_with("team/") || stripped.starts_with("org/") {
                return false;
            }
            self.users
                .get(stripped)
                .is_some_and(|u| u.status == UserStatus::Departed)
        } else {
            false
        }
    }

    /// Add a user definition (inserts or replaces).
    pub fn add_user(&mut self, user: UserDef) {
        self.users.insert(user.handle.clone(), user);
    }

    /// Mark a user as departed. Returns error if handle not found.
    pub fn depart_user(&mut self, handle: &str) -> Result<()> {
        let user = self
            .users
            .get_mut(handle)
            .ok_or_else(|| Error::FrontmatterParse(format!("unknown user: '{handle}'")))?;
        user.status = UserStatus::Departed;
        Ok(())
    }

    /// Mark a team as deprecated. Returns error if team not found.
    pub fn deprecate_team(&mut self, id: &str) -> Result<()> {
        let team = self
            .teams
            .get_mut(id)
            .ok_or_else(|| Error::FrontmatterParse(format!("unknown team: '{id}'")))?;
        team.status = TeamStatus::Deprecated;
        Ok(())
    }

    /// Add a team definition (inserts or replaces).
    pub fn add_team(&mut self, team: TeamDef) {
        self.teams.insert(team.id.clone(), team);
    }

    /// Add an org definition (inserts or replaces).
    pub fn add_org(&mut self, org: OrgDef) {
        self.orgs.insert(org.id.clone(), org);
    }

    /// Serialize to a KDL string (sorted deterministically).
    pub fn to_kdl_string(&self) -> String {
        let mut doc = KdlDocument::new();

        // Jira integrations first
        for j in &self.jira {
            let mut node = KdlNode::new("jira");
            node.push(KdlEntry::new(KdlValue::String(j.prefix.clone())));
            let mut children = KdlDocument::new();
            let mut url_node = KdlNode::new("url");
            url_node.push(KdlEntry::new(KdlValue::String(j.url.clone())));
            children.nodes_mut().push(url_node);
            node.set_children(children);
            doc.nodes_mut().push(node);
        }

        // Orgs (sorted by id)
        let mut org_ids: Vec<&String> = self.orgs.keys().collect();
        org_ids.sort();
        for id in org_ids {
            doc.nodes_mut().push(org_to_kdl_node(&self.orgs[id]));
        }

        // Teams sorted by id
        let mut team_ids: Vec<&String> = self.teams.keys().collect();
        team_ids.sort();
        for id in team_ids {
            doc.nodes_mut().push(team_to_kdl_node(&self.teams[id]));
        }

        // Users sorted by handle
        let mut handles: Vec<&String> = self.users.keys().collect();
        handles.sort();
        for handle in handles {
            doc.nodes_mut().push(user_to_kdl_node(&self.users[handle]));
        }

        doc.autoformat();
        format!("{doc}")
    }

    /// Write user config to a file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_kdl_string();
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }
}

/// Get the first positional (unnamed) string argument from a KDL node.
fn get_string_arg(node: &KdlNode) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string())
}

/// Collect all positional string arguments from a KDL node.
fn get_string_args(node: &KdlNode) -> Vec<String> {
    node.entries()
        .iter()
        .filter(|e| e.name().is_none())
        .filter_map(|e| e.value().as_string().map(|s| s.to_string()))
        .collect()
}

fn parse_user_def(node: &KdlNode) -> Result<UserDef> {
    let handle = get_string_arg(node)
        .ok_or_else(|| Error::FrontmatterParse("user node missing handle argument".into()))?;

    let children = node
        .children()
        .ok_or_else(|| Error::FrontmatterParse(format!("user '{handle}' must have a body")))?;

    let mut name = None;
    let mut title = None;
    let mut email = None;
    let mut teams = Vec::new();
    let mut org = None;
    let mut status = UserStatus::Active;
    let mut kind = EntityKind::Internal;
    let mut extra = BTreeMap::new();

    for child in children.nodes() {
        let key = child.name().value();
        match key {
            "name" => name = get_string_arg(child),
            "title" => title = get_string_arg(child),
            "email" => email = get_string_arg(child),
            "teams" => teams = get_string_args(child),
            "org" => org = get_string_arg(child),
            "status" => {
                if let Some(val) = get_string_arg(child) {
                    status = val.parse()?;
                }
            }
            "kind" => {
                if let Some(val) = get_string_arg(child) {
                    kind = val.parse()?;
                }
            }
            _ => {
                if let Some(val) = get_string_value(child) {
                    extra.insert(key.to_string(), val);
                }
            }
        }
    }

    Ok(UserDef {
        handle,
        name,
        title,
        email,
        teams,
        org,
        status,
        kind,
        extra,
    })
}

fn parse_team_def(node: &KdlNode) -> Result<TeamDef> {
    let id = get_string_arg(node)
        .ok_or_else(|| Error::FrontmatterParse("team node missing id argument".into()))?;

    let children = node
        .children()
        .ok_or_else(|| Error::FrontmatterParse(format!("team '{id}' must have a body")))?;

    let mut name = None;
    let mut lead = None;
    let mut parent = None;
    let mut teams = Vec::new();
    let mut org = None;
    let mut status = TeamStatus::Active;
    let mut kind = EntityKind::Internal;
    let mut extra = BTreeMap::new();

    for child in children.nodes() {
        let key = child.name().value();
        match key {
            "name" => name = get_string_arg(child),
            "lead" => lead = get_string_arg(child),
            "parent" => parent = get_string_arg(child),
            "teams" => teams = get_string_args(child),
            "org" => org = get_string_arg(child),
            "status" => {
                if let Some(val) = get_string_arg(child) {
                    status = val.parse()?;
                }
            }
            "kind" => {
                if let Some(val) = get_string_arg(child) {
                    kind = val.parse()?;
                }
            }
            _ => {
                if let Some(val) = get_string_value(child) {
                    extra.insert(key.to_string(), val);
                }
            }
        }
    }

    Ok(TeamDef {
        id,
        name,
        lead,
        parent,
        teams,
        org,
        status,
        kind,
        extra,
    })
}

fn parse_org_def(node: &KdlNode) -> Result<OrgDef> {
    let id = get_string_arg(node)
        .ok_or_else(|| Error::FrontmatterParse("org node missing id argument".into()))?;

    let children = node
        .children()
        .ok_or_else(|| Error::FrontmatterParse(format!("org '{id}' must have a body")))?;

    let mut name = None;
    let mut parent = None;
    let mut primary = false;
    let mut extra = BTreeMap::new();

    for child in children.nodes() {
        let key = child.name().value();
        match key {
            "name" => name = get_string_arg(child),
            "parent" => parent = get_string_arg(child),
            "primary" => primary = get_bool_value(child).unwrap_or(false),
            _ => {
                if let Some(val) = get_string_value(child) {
                    extra.insert(key.to_string(), val);
                }
            }
        }
    }

    Ok(OrgDef {
        id,
        name,
        parent,
        primary,
        extra,
    })
}

/// Extract a string value from a child node (first positional arg).
fn get_string_value(node: &KdlNode) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| match e.value() {
            KdlValue::String(s) => Some(s.to_string()),
            KdlValue::Integer(n) => Some(n.to_string()),
            KdlValue::Bool(b) => Some(b.to_string()),
            _ => None,
        })
}

fn get_bool_value(node: &KdlNode) -> Option<bool> {
    node.entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_bool())
}

// --- KDL serialization helpers ---

fn make_string_child(name: &str, value: &str) -> KdlNode {
    let mut node = KdlNode::new(name);
    node.entries_mut()
        .push(KdlEntry::new(KdlValue::String(value.to_string())));
    node
}

fn make_strings_child(name: &str, values: &[String]) -> KdlNode {
    let mut node = KdlNode::new(name);
    for v in values {
        node.entries_mut()
            .push(KdlEntry::new(KdlValue::String(v.clone())));
    }
    node
}

fn user_to_kdl_node(user: &UserDef) -> KdlNode {
    let mut node = KdlNode::new("user");
    node.entries_mut()
        .push(KdlEntry::new(KdlValue::String(user.handle.clone())));
    let children = node.ensure_children();
    if let Some(ref name) = user.name {
        children.nodes_mut().push(make_string_child("name", name));
    }
    if let Some(ref title) = user.title {
        children.nodes_mut().push(make_string_child("title", title));
    }
    if let Some(ref email) = user.email {
        children.nodes_mut().push(make_string_child("email", email));
    }
    if !user.teams.is_empty() {
        children
            .nodes_mut()
            .push(make_strings_child("teams", &user.teams));
    }
    if let Some(ref org) = user.org {
        children.nodes_mut().push(make_string_child("org", org));
    }
    if user.status == UserStatus::Departed {
        children
            .nodes_mut()
            .push(make_string_child("status", "departed"));
    }
    if user.kind == EntityKind::External {
        children
            .nodes_mut()
            .push(make_string_child("kind", "external"));
    }
    for (key, val) in &user.extra {
        children.nodes_mut().push(make_string_child(key, val));
    }
    node
}

fn team_to_kdl_node(team: &TeamDef) -> KdlNode {
    let mut node = KdlNode::new("team");
    node.entries_mut()
        .push(KdlEntry::new(KdlValue::String(team.id.clone())));
    let children = node.ensure_children();
    if let Some(ref name) = team.name {
        children.nodes_mut().push(make_string_child("name", name));
    }
    if let Some(ref lead) = team.lead {
        children.nodes_mut().push(make_string_child("lead", lead));
    }
    if let Some(ref parent) = team.parent {
        children
            .nodes_mut()
            .push(make_string_child("parent", parent));
    }
    if !team.teams.is_empty() {
        children
            .nodes_mut()
            .push(make_strings_child("teams", &team.teams));
    }
    if let Some(ref org) = team.org {
        children.nodes_mut().push(make_string_child("org", org));
    }
    if team.status == TeamStatus::Deprecated {
        children
            .nodes_mut()
            .push(make_string_child("status", "deprecated"));
    }
    if team.kind == EntityKind::External {
        children
            .nodes_mut()
            .push(make_string_child("kind", "external"));
    }
    for (key, val) in &team.extra {
        children.nodes_mut().push(make_string_child(key, val));
    }
    node
}

fn org_to_kdl_node(org: &OrgDef) -> KdlNode {
    let mut node = KdlNode::new("org");
    node.entries_mut()
        .push(KdlEntry::new(KdlValue::String(org.id.clone())));
    let children = node.ensure_children();
    if let Some(ref name) = org.name {
        children.nodes_mut().push(make_string_child("name", name));
    }
    if let Some(ref parent) = org.parent {
        children
            .nodes_mut()
            .push(make_string_child("parent", parent));
    }
    if org.primary {
        let mut pnode = KdlNode::new("primary");
        pnode
            .entries_mut()
            .push(KdlEntry::new(KdlValue::Bool(true)));
        children.nodes_mut().push(pnode);
    }
    for (key, val) in &org.extra {
        children.nodes_mut().push(make_string_child(key, val));
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> OrgConfig {
        OrgConfig::from_str(
            r##"
user "onni" {
    name "Onni Example"
    email "onni@example.com"
    teams "platform" "leadership"
    role "staff-engineer"
}

user "alice" {
    name "Alice Smith"
    email "alice@example.com"
    teams "platform"
}

user "bob" {
    name "Bob Jones"
    teams "security"
}

team "platform" {
    name "Platform Team"
    slack "#platform"
    lead "onni"
}

team "security" {
    name "Security Team"
    slack "#security"
}

team "leadership" {
    name "Leadership"
}

team "engineering" {
    name "Engineering"
    teams "platform" "security"
}
"##,
        )
        .unwrap()
    }

    #[test]
    fn test_load_users() {
        let config = test_config();
        assert_eq!(config.users.len(), 3);
        assert_eq!(config.teams.len(), 4);

        let onni = &config.users["onni"];
        assert_eq!(onni.name.as_deref(), Some("Onni Example"));
        assert_eq!(onni.email.as_deref(), Some("onni@example.com"));
        assert!(onni.teams.contains(&"platform".to_string()));
        assert_eq!(onni.extra["role"].as_str(), "staff-engineer");
    }

    #[test]
    fn test_load_teams() {
        let config = test_config();
        let platform = &config.teams["platform"];
        assert_eq!(platform.name.as_deref(), Some("Platform Team"));
        assert_eq!(platform.extra["slack"].as_str(), "#platform");
        assert_eq!(platform.lead.as_deref(), Some("onni"));

        let eng = &config.teams["engineering"];
        assert!(eng.teams.contains(&"platform".to_string()));
        assert!(eng.teams.contains(&"security".to_string()));
    }

    #[test]
    fn test_valid_refs() {
        let config = test_config();
        assert!(config.is_valid_ref("@onni"));
        assert!(config.is_valid_ref("@alice"));
        assert!(config.is_valid_ref("@team/platform"));
        assert!(config.is_valid_ref("@team/engineering"));
        assert!(!config.is_valid_ref("@unknown"));
        assert!(!config.is_valid_ref("@team/unknown"));
        assert!(!config.is_valid_ref("onni")); // missing @
    }

    #[test]
    fn test_is_valid_user() {
        let config = test_config();
        assert!(config.is_valid_user("@onni"));
        assert!(!config.is_valid_user("@team/platform"));
        assert!(!config.is_valid_user("@unknown"));
    }

    #[test]
    fn test_expand_team_members() {
        let config = test_config();

        // Platform team: onni + alice
        let platform = config.expand_team_members("platform");
        assert!(platform.contains("onni"));
        assert!(platform.contains("alice"));
        assert!(!platform.contains("bob"));

        // Engineering team: platform(onni, alice) + security(bob)
        let eng = config.expand_team_members("engineering");
        assert!(eng.contains("onni"));
        assert!(eng.contains("alice"));
        assert!(eng.contains("bob"));
    }

    #[test]
    fn test_cycle_protection() {
        // Team A contains B, B contains A
        let config = OrgConfig::from_str(
            r#"
user "x" {
    name "X"
    teams "a"
}
team "a" {
    name "A"
    teams "b"
}
team "b" {
    name "B"
    teams "a"
}
"#,
        )
        .unwrap();

        // Should not infinite loop
        let members = config.expand_team_members("a");
        assert!(members.contains("x"));
    }

    #[test]
    fn test_all_handles_and_names() {
        let config = test_config();
        let handles = config.all_user_handles();
        assert!(handles.contains(&"@onni".to_string()));
        assert!(handles.contains(&"@alice".to_string()));

        let teams = config.all_team_names();
        assert!(teams.contains(&"@team/platform".to_string()));
        assert!(teams.contains(&"@team/engineering".to_string()));
    }

    #[test]
    fn test_parse_orgs() {
        let config = org_config();
        assert_eq!(config.orgs.len(), 2);
        let acme = &config.orgs["acme"];
        assert_eq!(acme.name.as_deref(), Some("Acme Corp"));
        assert!(acme.parent.is_none());
        assert!(acme.primary);

        let acme_eu = &config.orgs["acme-eu"];
        assert_eq!(acme_eu.parent.as_deref(), Some("acme"));
        assert!(!acme_eu.primary);
    }

    #[test]
    fn test_no_orgs_backward_compat() {
        let config = test_config();
        assert!(config.orgs.is_empty());
    }

    #[test]
    fn test_is_valid_org() {
        let config = org_config();
        assert!(config.is_valid_org("@org/acme"));
        assert!(config.is_valid_org("@org/acme-eu"));
        assert!(!config.is_valid_org("@org/unknown"));
        assert!(!config.is_valid_org("@team/acme"));
        assert!(!config.is_valid_org("@acme"));
    }

    #[test]
    fn test_org_ref_in_is_valid_ref() {
        let config = org_config();
        assert!(config.is_valid_ref("@org/acme"));
        assert!(config.is_valid_ref("@org/acme-eu"));
        assert!(!config.is_valid_ref("@org/unknown"));
        // User/team still work
        assert!(config.is_valid_ref("@onni"));
        assert!(config.is_valid_ref("@team/platform"));
    }

    #[test]
    fn test_all_org_names() {
        let config = org_config();
        let orgs = config.all_org_names();
        assert!(orgs.contains(&"@org/acme".to_string()));
        assert!(orgs.contains(&"@org/acme-eu".to_string()));
    }

    #[test]
    fn test_expand_org() {
        let config = org_config();
        let expanded = config.expand_org("acme");
        assert!(expanded.contains("acme"));
        assert!(expanded.contains("acme-eu")); // child of acme
    }

    #[test]
    fn test_expand_org_leaf() {
        let config = org_config();
        let expanded = config.expand_org("acme-eu");
        assert!(expanded.contains("acme-eu"));
        assert!(!expanded.contains("acme")); // parent, not child
    }

    #[test]
    fn test_org_cycle_protection() {
        let config = OrgConfig::from_str(
            r#"
org "a" {
    name "A"
    parent "b"
}
org "b" {
    name "B"
    parent "a"
}
"#,
        )
        .unwrap();
        // Should not infinite loop
        let expanded = config.expand_org("a");
        assert!(expanded.contains("a"));
    }

    #[test]
    fn test_user_effective_org_explicit() {
        let config = org_config();
        // onni has explicit org "acme"
        assert_eq!(config.user_effective_org("onni"), Some("acme".to_string()));
    }

    #[test]
    fn test_user_effective_org_from_team() {
        let config = org_config();
        // alice has no explicit org, but is on "platform" which has org "acme-eu"
        assert_eq!(
            config.user_effective_org("alice"),
            Some("acme-eu".to_string())
        );
    }

    #[test]
    fn test_user_effective_org_none() {
        let config = org_config();
        // bob has no org and is on "security" which has no org
        assert_eq!(config.user_effective_org("bob"), None);
    }

    #[test]
    fn test_users_in_org() {
        let config = org_config();
        // "acme" includes acme + acme-eu (child)
        // onni -> acme (explicit), alice -> acme-eu (via team)
        let users = config.users_in_org("acme");
        assert!(users.contains("onni"));
        assert!(users.contains("alice"));
        assert!(!users.contains("bob"));
    }

    #[test]
    fn test_users_in_org_leaf() {
        let config = org_config();
        let users = config.users_in_org("acme-eu");
        assert!(!users.contains("onni")); // onni is in acme, not acme-eu
        assert!(users.contains("alice"));
    }

    #[test]
    fn test_user_org_on_user_and_team() {
        let config = org_config();
        assert_eq!(config.users["onni"].org.as_deref(), Some("acme"));
        assert_eq!(config.teams["platform"].org.as_deref(), Some("acme-eu"));
    }

    #[test]
    fn test_user_status_roundtrip() {
        let active: UserStatus = "active".parse().unwrap();
        assert_eq!(active, UserStatus::Active);
        assert_eq!(active.to_string(), "active");

        let departed: UserStatus = "departed".parse().unwrap();
        assert_eq!(departed, UserStatus::Departed);
        assert_eq!(departed.to_string(), "departed");

        // Case insensitive
        let dep: UserStatus = "Departed".parse().unwrap();
        assert_eq!(dep, UserStatus::Departed);

        // Invalid
        assert!("unknown".parse::<UserStatus>().is_err());
    }

    #[test]
    fn test_user_status_default() {
        assert_eq!(UserStatus::default(), UserStatus::Active);
    }

    #[test]
    fn test_parse_departed_user() {
        let config = OrgConfig::from_str(
            r#"
user "alice" {
    name "Alice"
    status "departed"
}
user "bob" {
    name "Bob"
}
"#,
        )
        .unwrap();
        assert_eq!(config.users["alice"].status, UserStatus::Departed);
        assert_eq!(config.users["bob"].status, UserStatus::Active);
    }

    #[test]
    fn test_is_departed_user() {
        let config = OrgConfig::from_str(
            r#"
user "alice" {
    name "Alice"
    status "departed"
}
user "bob" {
    name "Bob"
}
team "platform" {
    name "Platform"
}
"#,
        )
        .unwrap();
        assert!(config.is_departed_user("@alice"));
        assert!(!config.is_departed_user("@bob"));
        assert!(!config.is_departed_user("@team/platform"));
        assert!(!config.is_departed_user("@unknown"));
    }

    #[test]
    fn test_depart_user() {
        let mut config = test_config();
        assert_eq!(config.users["onni"].status, UserStatus::Active);
        config.depart_user("onni").unwrap();
        assert_eq!(config.users["onni"].status, UserStatus::Departed);
        assert!(config.depart_user("nonexistent").is_err());
    }

    #[test]
    fn test_add_user() {
        let mut config = test_config();
        let count = config.users.len();
        config.add_user(UserDef {
            handle: "newuser".into(),
            name: Some("New User".into()),
            title: None,
            email: None,
            teams: vec!["platform".into()],
            org: None,
            status: UserStatus::Active,
            kind: EntityKind::Internal,
            extra: BTreeMap::new(),
        });
        assert_eq!(config.users.len(), count + 1);
        assert!(config.is_valid_ref("@newuser"));
    }

    #[test]
    fn test_add_team() {
        let mut config = test_config();
        let count = config.teams.len();
        config.add_team(TeamDef {
            id: "newteam".into(),
            name: Some("New Team".into()),
            lead: None,
            parent: None,
            teams: vec![],
            org: None,
            status: TeamStatus::Active,
            kind: EntityKind::Internal,
            extra: BTreeMap::new(),
        });
        assert_eq!(config.teams.len(), count + 1);
        assert!(config.is_valid_ref("@team/newteam"));
    }

    #[test]
    fn test_add_org() {
        let mut config = org_config();
        let count = config.orgs.len();
        config.add_org(OrgDef {
            id: "neworg".into(),
            name: Some("New Org".into()),
            parent: None,
            primary: false,
            extra: BTreeMap::new(),
        });
        assert_eq!(config.orgs.len(), count + 1);
        assert!(config.is_valid_org("@org/neworg"));
    }

    #[test]
    fn test_kdl_serialization_roundtrip() {
        let config = org_config();
        let kdl = config.to_kdl_string();
        let parsed = OrgConfig::from_str(&kdl).unwrap();
        assert_eq!(parsed.users.len(), config.users.len());
        assert_eq!(parsed.teams.len(), config.teams.len());
        assert_eq!(parsed.orgs.len(), config.orgs.len());
        // Check a user survived roundtrip
        assert_eq!(parsed.users["onni"].name.as_deref(), Some("Onni Example"));
    }

    #[test]
    fn test_kdl_serialization_departed_status() {
        let mut config = OrgConfig::from_str(
            r#"
user "alice" {
    name "Alice"
}
"#,
        )
        .unwrap();
        config.depart_user("alice").unwrap();
        let kdl = config.to_kdl_string();
        assert!(kdl.contains("departed"));
        // Roundtrip
        let parsed = OrgConfig::from_str(&kdl).unwrap();
        assert_eq!(parsed.users["alice"].status, UserStatus::Departed);
    }

    #[test]
    fn test_kdl_serialization_active_omits_status() {
        let config = OrgConfig::from_str(
            r#"
user "bob" {
    name "Bob"
}
"#,
        )
        .unwrap();
        let kdl = config.to_kdl_string();
        // Active status should not appear in output
        assert!(!kdl.contains("status"));
    }

    #[test]
    fn test_entity_kind_roundtrip() {
        let internal: EntityKind = "internal".parse().unwrap();
        assert_eq!(internal, EntityKind::Internal);
        assert_eq!(internal.to_string(), "internal");

        let external: EntityKind = "external".parse().unwrap();
        assert_eq!(external, EntityKind::External);
        assert_eq!(external.to_string(), "external");

        // Case insensitive
        let ext: EntityKind = "External".parse().unwrap();
        assert_eq!(ext, EntityKind::External);

        // Invalid
        assert!("unknown".parse::<EntityKind>().is_err());
    }

    #[test]
    fn test_entity_kind_default() {
        assert_eq!(EntityKind::default(), EntityKind::Internal);
    }

    #[test]
    fn test_parse_title_and_kind() {
        let config = OrgConfig::from_str(
            r#"
user "jane" {
    name "Jane Smith"
    title "VP Engineering"
    kind "external"
}
user "bob" {
    name "Bob"
}
team "contractors" {
    name "External Contractors"
    kind "external"
    lead "jane"
    parent "engineering"
}
team "engineering" {
    name "Engineering"
}
"#,
        )
        .unwrap();

        let jane = &config.users["jane"];
        assert_eq!(jane.title.as_deref(), Some("VP Engineering"));
        assert_eq!(jane.kind, EntityKind::External);

        let bob = &config.users["bob"];
        assert!(bob.title.is_none());
        assert_eq!(bob.kind, EntityKind::Internal);

        let contractors = &config.teams["contractors"];
        assert_eq!(contractors.kind, EntityKind::External);
        assert_eq!(contractors.lead.as_deref(), Some("jane"));
        assert_eq!(contractors.parent.as_deref(), Some("engineering"));

        let eng = &config.teams["engineering"];
        assert_eq!(eng.kind, EntityKind::Internal);
        assert!(eng.lead.is_none());
        assert!(eng.parent.is_none());
    }

    #[test]
    fn test_title_kind_serialization_roundtrip() {
        let config = OrgConfig::from_str(
            r#"
user "jane" {
    name "Jane"
    title "CTO"
    kind "external"
}
team "ext" {
    name "External"
    lead "jane"
    parent "engineering"
    kind "external"
}
team "engineering" {
    name "Engineering"
}
"#,
        )
        .unwrap();

        let kdl = config.to_kdl_string();
        assert!(kdl.contains("title"));
        assert!(kdl.contains("CTO"));
        assert!(
            kdl.contains("kind external"),
            "missing kind external in:\n{kdl}"
        );
        assert!(kdl.contains("lead jane"), "missing lead jane in:\n{kdl}");
        assert!(
            kdl.contains("parent engineering"),
            "missing parent engineering in:\n{kdl}"
        );

        // Internal kind should NOT appear in output
        assert_eq!(
            kdl.matches("kind ").count(),
            2,
            "only external entities should emit kind in:\n{kdl}"
        );

        // Roundtrip
        let parsed = OrgConfig::from_str(&kdl).unwrap();
        assert_eq!(parsed.users["jane"].title.as_deref(), Some("CTO"));
        assert_eq!(parsed.users["jane"].kind, EntityKind::External);
        assert_eq!(parsed.teams["ext"].lead.as_deref(), Some("jane"));
        assert_eq!(parsed.teams["ext"].parent.as_deref(), Some("engineering"));
        assert_eq!(parsed.teams["ext"].kind, EntityKind::External);
        assert_eq!(parsed.teams["engineering"].kind, EntityKind::Internal);
    }

    fn org_config() -> OrgConfig {
        OrgConfig::from_str(
            r##"
org "acme" {
    name "Acme Corp"
    primary #true
}

org "acme-eu" {
    name "Acme EU"
    parent "acme"
}

user "onni" {
    name "Onni Example"
    email "onni@example.com"
    teams "platform" "leadership"
    org "acme"
}

user "alice" {
    name "Alice Smith"
    email "alice@example.com"
    teams "platform"
}

user "bob" {
    name "Bob Jones"
    teams "security"
}

team "platform" {
    name "Platform Team"
    org "acme-eu"
}

team "security" {
    name "Security Team"
}

team "leadership" {
    name "Leadership"
}

team "engineering" {
    name "Engineering"
    teams "platform" "security"
}
"##,
        )
        .unwrap()
    }
}
