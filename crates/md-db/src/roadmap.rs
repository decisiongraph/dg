use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::export::{encode_attr, encode_text};
use crate::graph::DocGraph;
use crate::schema::Schema;
use crate::users::OrgConfig;

// ── Data model ──────────────────────────────────────────────────────────────

/// A fiscal quarter (e.g. 2026-Q1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Quarter {
    pub year: i32,
    pub q: u8,
}

impl Quarter {
    pub fn new(year: i32, q: u8) -> Self {
        Self { year, q }
    }

    /// Parse "YYYY-QN" string.
    pub fn parse(s: &str) -> Option<Self> {
        let (year_str, q_str) = s.split_once("-Q").or_else(|| s.split_once("-q"))?;
        let year = year_str.parse::<i32>().ok()?;
        let q = q_str.parse::<u8>().ok()?;
        if (1..=4).contains(&q) {
            Some(Self { year, q })
        } else {
            None
        }
    }

    /// Quarter from a YYYY-MM-DD date string.
    pub fn from_date(date: &str) -> Option<Self> {
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() < 2 {
            return None;
        }
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u8>().ok()?;
        let q = match month {
            1..=3 => 1,
            4..=6 => 2,
            7..=9 => 3,
            10..=12 => 4,
            _ => return None,
        };
        Some(Self { year, q })
    }

    /// Advance by n quarters (can be negative via offset_back).
    pub fn offset(&self, n: i32) -> Self {
        let total = (self.year * 4 + self.q as i32 - 1) + n;
        let year = total.div_euclid(4);
        let q = (total.rem_euclid(4) + 1) as u8;
        Self { year, q }
    }

    pub fn label(&self) -> String {
        format!("Q{} {}", self.q, self.year)
    }

    pub fn id(&self) -> String {
        format!("{}-Q{}", self.year, self.q)
    }

    /// First calendar month of this quarter (1-based).
    pub fn first_month(&self) -> u32 {
        (self.q as u32 - 1) * 3 + 1
    }

    /// Last calendar month of this quarter (1-based).
    pub fn last_month(&self) -> u32 {
        self.q as u32 * 3
    }
}

impl std::fmt::Display for Quarter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-Q{}", self.year, self.q)
    }
}

/// Full roadmap data.
#[derive(Debug, Serialize)]
pub struct RoadmapData {
    pub quarters: BTreeMap<Quarter, QuarterData>,
    pub backlog: Vec<RoadmapItem>,
    pub team_capacities: BTreeMap<String, Vec<(Quarter, CapacitySlots)>>,
    /// Map from team ID to display name (e.g. "it-department" → "IT Department").
    pub team_names: BTreeMap<String, String>,
    pub generated_at: String,
}

impl RoadmapData {
    /// True when the roadmap contains no items at all (no quarter items and no backlog).
    pub fn is_empty(&self) -> bool {
        self.backlog.is_empty() && self.quarters.values().all(|q| q.items.is_empty())
    }
}

/// Data for a single quarter.
#[derive(Debug, Default, Serialize)]
pub struct QuarterData {
    pub items: Vec<RoadmapItem>,
    pub capacity_by_team: BTreeMap<String, CapacitySlots>,
    pub used_by_team: BTreeMap<String, CapacitySlots>,
}

/// A roadmap item (typically an OPP with linked docs).
#[derive(Debug, Clone, Serialize)]
pub struct RoadmapItem {
    pub id: String,
    pub title: String,
    pub doc_type: String,
    pub status: String,
    pub effort: Option<String>,
    pub priority: Option<String>,
    pub impact: Option<String>,
    pub owner: Option<String>,
    pub team: Option<String>,
    pub date: Option<String>,
    pub linked_docs: Vec<LinkedDoc>,
}

/// A document linked to a roadmap item (ADR/POL triggered by an OPP).
#[derive(Debug, Clone, Serialize)]
pub struct LinkedDoc {
    pub id: String,
    pub doc_type: String,
    pub title: String,
    pub status: String,
}

/// Capacity slots per effort size.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapacitySlots {
    #[serde(default)]
    pub large: u8,
    #[serde(default)]
    pub medium: u8,
    #[serde(default)]
    pub small: u8,
    #[serde(default)]
    pub note: Option<String>,
}

impl CapacitySlots {
    fn add_effort(&mut self, effort: &str) {
        match effort.to_lowercase().as_str() {
            "large" | "l" => self.large += 1,
            "medium" | "m" => self.medium += 1,
            "small" | "s" => self.small += 1,
            _ => self.medium += 1, // default
        }
    }
}

// ── Config ──────────────────────────────────────────────────────────────────

/// Roadmap config loaded from .dg/roadmap.yaml.
#[derive(Debug, Deserialize)]
pub struct RoadmapConfig {
    #[serde(default)]
    pub capacity: CapacityConfig,
    #[serde(default)]
    pub display: DisplayConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct CapacityConfig {
    #[serde(default)]
    pub default: Option<CapacitySlots>,
    #[serde(default)]
    pub teams: BTreeMap<String, CapacitySlots>,
    #[serde(default)]
    pub overrides: BTreeMap<String, BTreeMap<String, CapacitySlots>>,
}

#[derive(Debug, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_past_quarters")]
    pub past_quarters: u8,
    #[serde(default = "default_future_quarters")]
    pub future_quarters: u8,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            past_quarters: default_past_quarters(),
            future_quarters: default_future_quarters(),
        }
    }
}

fn default_past_quarters() -> u8 {
    4
}
fn default_future_quarters() -> u8 {
    4
}

impl RoadmapConfig {
    pub fn from_file(path: &Path) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Get capacity for a team in a given quarter.
    fn team_capacity(&self, team: &str, quarter: &Quarter) -> CapacitySlots {
        // Check quarter-specific override first
        if let Some(q_overrides) = self.capacity.overrides.get(&quarter.id()) {
            if let Some(slots) = q_overrides.get(team) {
                return slots.clone();
            }
        }
        // Then team default
        if let Some(slots) = self.capacity.teams.get(team) {
            return slots.clone();
        }
        // Then global default
        self.capacity.default.clone().unwrap_or(CapacitySlots {
            large: 1,
            medium: 2,
            small: 3,
            note: None,
        })
    }
}

// ── Builder ─────────────────────────────────────────────────────────────────

/// Status categories for quarter assignment.
fn is_completed(status: &str) -> bool {
    matches!(
        status.to_lowercase().as_str(),
        "completed" | "resolved" | "declined" | "rejected" | "deprecated" | "superseded"
    )
}

fn is_active(status: &str) -> bool {
    matches!(
        status.to_lowercase().as_str(),
        "pursuing" | "validating" | "active" | "accepted" | "in-progress" | "in_progress"
    )
}

fn is_backlog(status: &str) -> bool {
    matches!(
        status.to_lowercase().as_str(),
        "identified" | "proposed" | "draft"
    )
}

/// Resolve owner handle to team name.
fn resolve_team(owner: &str, users: Option<&OrgConfig>) -> Option<String> {
    let users = users?;
    let handle = owner.strip_prefix('@').unwrap_or(owner);
    let user = users.users.get(handle)?;
    user.teams.first().cloned()
}

/// Priority sort order (lower = higher priority).
fn priority_rank(p: Option<&str>) -> u8 {
    match p.map(|s| s.to_lowercase()).as_deref() {
        Some("critical") => 0,
        Some("high") => 1,
        Some("medium") => 2,
        Some("low") => 3,
        _ => 4,
    }
}

/// Build roadmap data from documents.
///
/// `status_history` is optional git-derived history mapping doc_id -> [(from, to, date)].
/// When provided, completed items are placed in the quarter of their completion date.
#[allow(clippy::too_many_arguments)]
pub fn build_roadmap(
    dir: &Path,
    schema: &Schema,
    config: Option<&RoadmapConfig>,
    users: Option<&OrgConfig>,
    today: &str,
    past_quarters: u8,
    future_quarters: u8,
    #[cfg(feature = "git")] status_history: Option<
        &BTreeMap<String, Vec<crate::history::StatusTransition>>,
    >,
) -> crate::error::Result<RoadmapData> {
    let graph = DocGraph::build(dir, schema)?;
    let current_q = Quarter::from_date(today).unwrap_or(Quarter::new(2026, 1));

    let start_q = current_q.offset(-(past_quarters as i32));
    let end_q = current_q.offset(future_quarters as i32);

    // Initialize quarter buckets
    let mut quarters: BTreeMap<Quarter, QuarterData> = BTreeMap::new();
    let mut q = start_q.clone();
    while q <= end_q {
        quarters.insert(q.clone(), QuarterData::default());
        q = q.offset(1);
    }

    let mut backlog: Vec<RoadmapItem> = Vec::new();

    // Collect all OPP nodes (check frontmatter type or infer from folder)
    let opp_nodes: Vec<_> = graph
        .nodes
        .values()
        .filter(|n| {
            let explicit = n
                .doc_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("opp"))
                .unwrap_or(false);
            if explicit {
                return true;
            }
            // Fallback: infer type from folder path
            crate::validation::infer_type_from_path(&n.path, dir, schema)
                .map(|t| t.eq_ignore_ascii_case("opp"))
                .unwrap_or(false)
        })
        .collect();

    for node in &opp_nodes {
        let doc = match Document::from_file(&node.path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let fm = match &doc.frontmatter {
            Some(fm) => fm,
            None => continue,
        };

        let status = fm
            .get_display("status")
            .unwrap_or_else(|| "identified".to_string());

        // Skip non-active work — completed/abandoned items clutter the roadmap
        let sl = status.to_lowercase();
        if matches!(
            sl.as_str(),
            "completed" | "resolved" | "deprecated" | "superseded" | "declined" | "rejected"
        ) {
            continue;
        }

        let owner = fm.get_display("owner");
        let team = owner.as_deref().and_then(|o| resolve_team(o, users));
        let date = fm.get_display("date");
        let effort = fm.get_display("effort");
        let priority = fm.get_display("priority");
        let impact = fm.get_display("impact");

        // Collect linked docs (ADRs/POLs that reference this OPP)
        let linked_docs = collect_linked_docs(&node.id, &graph, dir, schema);

        let item = RoadmapItem {
            id: node.id.clone(),
            title: node.title.clone().unwrap_or_else(|| node.id.clone()),
            doc_type: "opp".to_string(),
            status: status.clone(),
            effort,
            priority,
            impact,
            owner,
            team: team.clone(),
            date: date.clone(),
            linked_docs,
        };

        // Assign to quarter
        let target_quarter = assign_quarter(
            &item,
            &current_q,
            #[cfg(feature = "git")]
            status_history,
        );

        match target_quarter {
            Some(q) if quarters.contains_key(&q) => {
                let qd = quarters.get_mut(&q).unwrap();
                // Track capacity usage
                if let Some(ref team_id) = item.team {
                    let effort_str = item.effort.as_deref().unwrap_or("medium");
                    qd.used_by_team
                        .entry(team_id.clone())
                        .or_default()
                        .add_effort(effort_str);
                }
                qd.items.push(item);
            }
            Some(_) => {
                // Quarter outside display range — skip
            }
            None => {
                backlog.push(item);
            }
        }
    }

    // Sort items within each quarter by priority
    for qd in quarters.values_mut() {
        qd.items
            .sort_by_key(|item| priority_rank(item.priority.as_deref()));
    }
    backlog.sort_by_key(|item| priority_rank(item.priority.as_deref()));

    // Fill capacity data from config
    let mut team_capacities: BTreeMap<String, Vec<(Quarter, CapacitySlots)>> = BTreeMap::new();
    if let Some(config) = config {
        // Collect all team IDs (from config + from items)
        let mut all_teams: std::collections::BTreeSet<String> =
            config.capacity.teams.keys().cloned().collect();
        for qd in quarters.values() {
            for item in &qd.items {
                if let Some(ref t) = item.team {
                    all_teams.insert(t.clone());
                }
            }
        }

        for team_id in &all_teams {
            let mut caps = Vec::new();
            let mut q = start_q.clone();
            while q <= end_q {
                let cap = config.team_capacity(team_id, &q);
                if let Some(qd) = quarters.get_mut(&q) {
                    qd.capacity_by_team.insert(team_id.clone(), cap.clone());
                }
                caps.push((q.clone(), cap));
                q = q.offset(1);
            }
            team_capacities.insert(team_id.clone(), caps);
        }
    }

    // Build team ID → display name map from user config
    let mut team_names: BTreeMap<String, String> = BTreeMap::new();
    if let Some(uc) = users {
        for (id, def) in &uc.teams {
            if let Some(ref name) = def.name {
                team_names.insert(id.clone(), name.clone());
            }
        }
    }

    Ok(RoadmapData {
        quarters,
        backlog,
        team_capacities,
        team_names,
        generated_at: today.to_string(),
    })
}

/// Determine which quarter an item belongs to.
fn assign_quarter(
    item: &RoadmapItem,
    current_q: &Quarter,
    #[cfg(feature = "git")] status_history: Option<
        &BTreeMap<String, Vec<crate::history::StatusTransition>>,
    >,
) -> Option<Quarter> {
    // Completed items: use git history completion date, fallback to frontmatter date
    if is_completed(&item.status) {
        #[cfg(feature = "git")]
        if let Some(history) = status_history {
            if let Some(transitions) = history.get(&item.id) {
                // Find the transition where status became completed
                for t in transitions.iter().rev() {
                    if is_completed(&t.to_status) {
                        if let Some(q) = Quarter::from_date(&t.date) {
                            return Some(q);
                        }
                    }
                }
            }
        }
        // Fallback: frontmatter date
        return item.date.as_deref().and_then(Quarter::from_date);
    }

    // Active items: current quarter or frontmatter date, whichever is later
    if is_active(&item.status) {
        let date_q = item.date.as_deref().and_then(Quarter::from_date);
        return Some(match date_q {
            Some(dq) if dq > *current_q => dq,
            _ => current_q.clone(),
        });
    }

    // Backlog (identified/proposed/draft)
    if is_backlog(&item.status) {
        return None;
    }

    // Unknown status: treat as backlog
    None
}

/// Resolve node type from frontmatter or folder inference.
fn resolve_node_type(node: &crate::graph::DocNode, dir: &Path, schema: &Schema) -> String {
    node.doc_type
        .clone()
        .or_else(|| crate::validation::infer_type_from_path(&node.path, dir, schema))
        .unwrap_or_else(|| "unknown".to_string())
        .to_lowercase()
}

/// Collect ADRs/POLs/INCs linked to an OPP via graph edges.
fn collect_linked_docs(
    opp_id: &str,
    graph: &DocGraph,
    dir: &Path,
    schema: &Schema,
) -> Vec<LinkedDoc> {
    let mut linked = Vec::new();

    // Outgoing refs from OPP
    for edge in graph.refs_from(opp_id) {
        if let Some(node) = graph.nodes.get(&edge.to) {
            let dtype = resolve_node_type(node, dir, schema);
            if dtype != "opp" {
                linked.push(LinkedDoc {
                    id: node.id.clone(),
                    doc_type: dtype,
                    title: node.title.clone().unwrap_or_else(|| node.id.clone()),
                    status: node.status.clone().unwrap_or_default(),
                });
            }
        }
    }

    // Incoming refs to OPP (backlinks)
    for edge in graph.refs_to(opp_id) {
        if let Some(node) = graph.nodes.get(&edge.from) {
            let dtype = resolve_node_type(node, dir, schema);
            if dtype != "opp" {
                // Deduplicate
                if !linked.iter().any(|l| l.id == node.id) {
                    linked.push(LinkedDoc {
                        id: node.id.clone(),
                        doc_type: dtype,
                        title: node.title.clone().unwrap_or_else(|| node.id.clone()),
                        status: node.status.clone().unwrap_or_default(),
                    });
                }
            }
        }
    }

    linked
}

// ── Date helpers (private) ──────────────────────────────────────────────────

/// Parse "YYYY-MM-DD" -> (year, month, day).
fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let year = parts[0].parse::<i32>().ok()?;
    let month = parts[1].parse::<u32>().ok()?;
    let day = parts[2].parse::<u32>().ok()?;
    if (1..=12).contains(&month) && (1..=31).contains(&day) {
        Some((year, month, day))
    } else {
        None
    }
}

/// Effort string -> bar duration in months.
fn effort_to_months(effort: Option<&str>) -> u32 {
    match effort.map(|s| s.to_lowercase()).as_deref() {
        Some("small" | "s") => 1,
        Some("large" | "l") => 6,
        _ => 3, // medium / unknown
    }
}

/// 0-based month index relative to timeline start.
fn month_index(year: i32, month: u32, start_year: i32, start_month: u32) -> i32 {
    (year - start_year) * 12 + month as i32 - start_month as i32
}

/// Days in a given month (handles leap years).
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Compute (start_year, start_month, end_year, end_month, total_months) from quarters map.
fn compute_time_range(quarters: &BTreeMap<Quarter, QuarterData>) -> (i32, u32, i32, u32, u32) {
    let first_q = quarters.keys().next().unwrap();
    let last_q = quarters.keys().next_back().unwrap();
    let start_year = first_q.year;
    let start_month = first_q.first_month();
    let end_year = last_q.year;
    let end_month = last_q.last_month();
    let total = month_index(end_year, end_month, start_year, start_month) as u32 + 1;
    (start_year, start_month, end_year, end_month, total)
}

/// Flatten all quarter items, group by team, sort by priority within each team.
/// Returns Vec<(team_name, Vec<&RoadmapItem>)> sorted alphabetically by team.
/// Items without a team go under "Unassigned".
fn group_items_by_team(data: &RoadmapData) -> Vec<(String, Vec<&RoadmapItem>)> {
    let mut groups: BTreeMap<String, Vec<&RoadmapItem>> = BTreeMap::new();
    for qdata in data.quarters.values() {
        for item in &qdata.items {
            let team_id = item.team.as_deref().unwrap_or("Unassigned");
            // Resolve team ID to display name
            let display = data
                .team_names
                .get(team_id)
                .cloned()
                .unwrap_or_else(|| team_id.to_string());
            groups.entry(display).or_default().push(item);
        }
    }
    // Sort by priority within each team
    for items in groups.values_mut() {
        items.sort_by_key(|item| priority_rank(item.priority.as_deref()));
    }
    groups.into_iter().collect()
}

// ── HTML Renderer ───────────────────────────────────────────────────────────

/// Render roadmap as a standalone HTML page with Gantt swimlane layout.
pub fn render_roadmap_html(
    data: &RoadmapData,
    current_quarter: &Quarter,
    schema: &crate::schema::Schema,
) -> String {
    let pill_css = generate_pill_css(schema);

    if data.quarters.is_empty() {
        return render_empty_roadmap(&data.generated_at, &pill_css);
    }

    let (start_year, start_month, _end_year, _end_month, total_months) =
        compute_time_range(&data.quarters);

    let today = &data.generated_at;

    let mut body = String::new();

    // Outer scroll container + inner positioned wrapper for correct today marker %
    body.push_str("<div class=\"gantt-scroll\">\n");
    body.push_str(&format!(
        "<div class=\"gantt-body\" style=\"--months: {total_months}\">\n"
    ));

    // Header: quarter groups + month labels
    body.push_str(&render_gantt_header(
        start_month,
        total_months,
        current_quarter,
        &data.quarters,
    ));

    // Team swimlanes
    let teams = group_items_by_team(data);
    for (team_name, items) in &teams {
        body.push_str(&render_team_swimlane(
            team_name,
            items,
            start_year,
            start_month,
            total_months,
            today,
        ));
    }

    // Today marker
    body.push_str(&render_today_marker(
        today,
        start_year,
        start_month,
        total_months,
    ));

    body.push_str("</div>\n</div>\n");

    // Backlog section (items with no date)
    if !data.backlog.is_empty() {
        body.push_str(&render_backlog_section(&data.backlog));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Tech Roadmap</title>
<style>{GANTT_CSS}{pill_css}</style>
</head>
<body>
<header>
<div class="header-text">
<h1>Tech Roadmap</h1>
<p class="generated">Generated {generated_at}</p>
</div>
</header>
{body}
<script>document.addEventListener('DOMContentLoaded',()=>{{const m=document.querySelector('.today-marker');if(m)m.scrollIntoView({{inline:'center',behavior:'instant'}})}});</script>
</body>
</html>
"#,
        pill_css = pill_css,
        generated_at = encode_text(&data.generated_at),
    )
}

fn render_empty_roadmap(generated_at: &str, pill_css: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Tech Roadmap</title>
<style>{GANTT_CSS}{pill_css}</style>
</head>
<body>
<header>
<div class="header-text">
<h1>Tech Roadmap</h1>
<p class="generated">Generated {generated_at}</p>
</div>
</header>
<p style="text-align:center;color:#94a3b8;padding:4rem">No roadmap items found.</p>
</body>
</html>
"#,
        pill_css = pill_css,
        generated_at = encode_text(generated_at),
    )
}

fn render_gantt_header(
    start_month: u32,
    total_months: u32,
    current_quarter: &Quarter,
    quarters: &BTreeMap<Quarter, QuarterData>,
) -> String {
    let mut html = String::from("<div class=\"gantt-header\">\n");

    // Row 1: quarter group labels
    html.push_str("<div class=\"header-row quarter-row\">\n");
    html.push_str("<div class=\"corner-spacer\"></div>\n");
    for q in quarters.keys() {
        let is_current = q == current_quarter;
        let current_class = if is_current { " current" } else { "" };
        html.push_str(&format!(
            "<div class=\"quarter-group{}\" style=\"grid-column: span 3\">{}</div>\n",
            current_class,
            encode_text(&q.label()),
        ));
    }
    html.push_str("</div>\n");

    // Row 2: month labels
    html.push_str("<div class=\"header-row month-row\">\n");
    html.push_str("<div class=\"corner-spacer\"></div>\n");
    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    for i in 0..total_months {
        let m = ((start_month - 1 + i) % 12) as usize;
        let is_qstart = (start_month + i - 1).is_multiple_of(3);
        let qstart_class = if is_qstart { " qstart" } else { "" };
        html.push_str(&format!(
            "<div class=\"month-cell{}\">{}</div>\n",
            qstart_class, month_names[m],
        ));
    }
    html.push_str("</div>\n");

    html.push_str("</div>\n");
    html
}

/// Returns (start_col, end_col) in CSS grid coordinates, or None if no date.
fn compute_bar_columns(
    item: &RoadmapItem,
    start_year: i32,
    start_month: u32,
    total_months: u32,
) -> Option<(u32, u32)> {
    let date = item.date.as_ref()?;
    let (y, m, _d) = parse_ymd(date)?;
    let duration = effort_to_months(item.effort.as_deref());
    let idx = month_index(y, m, start_year, start_month);
    let s = (idx + 2).max(2) as u32;
    let e = (idx as u32 + 2 + duration).min(total_months + 2);
    Some((s, e))
}

/// CSS grid column (2-based) for today's month.
fn today_month_col(today: &str, start_year: i32, start_month: u32) -> Option<u32> {
    let (ty, tm, _td) = parse_ymd(today)?;
    let idx = month_index(ty, tm, start_year, start_month);
    if idx < 0 {
        return None;
    }
    Some((idx + 2) as u32)
}

fn render_team_swimlane(
    team_name: &str,
    items: &[&RoadmapItem],
    start_year: i32,
    start_month: u32,
    total_months: u32,
    today: &str,
) -> String {
    let mut html = String::new();

    // Team header
    html.push_str("<div class=\"team-group\">\n");
    html.push_str(&format!(
        "<div class=\"team-header\"><div class=\"team-sticky\">{}</div></div>\n",
        encode_text(team_name),
    ));

    let today_col = today_month_col(today, start_year, start_month);

    // Pass 1: compute raw bar columns
    let mut bars: Vec<(&RoadmapItem, Option<(u32, u32)>)> = items
        .iter()
        .map(|item| {
            (
                *item,
                compute_bar_columns(item, start_year, start_month, total_months),
            )
        })
        .collect();

    // Pass 2: extend active overruns past today → today + 1 month; track watermark
    let mut watermark: u32 = 0;
    if let Some(tc) = today_col {
        for (item, cols) in &mut bars {
            if let Some((_, ref mut end_col)) = cols {
                if is_active(&item.status) && *end_col <= tc {
                    *end_col = (tc + 1).min(total_months + 2);
                    watermark = watermark.max(*end_col);
                }
            }
        }
    }

    // Pass 3: shift non-active/non-completed items that start before watermark
    if watermark > 0 {
        for (item, cols) in &mut bars {
            if let Some((ref mut start_col, ref mut end_col)) = cols {
                if !is_active(&item.status) && !is_completed(&item.status) && *start_col < watermark
                {
                    let span = *end_col - *start_col;
                    *start_col = watermark;
                    *end_col = (watermark + span).min(total_months + 2);
                }
            }
        }
    }

    // Pass 4: render rows
    for (item, cols) in &bars {
        match cols {
            Some((sc, ec)) => {
                html.push_str(&render_gantt_row(item, *sc, *ec, total_months, today_col));
            }
            None => {
                html.push_str(&render_no_bar_row(item));
            }
        }
    }

    html.push_str("</div>\n");
    html
}

fn render_gantt_row(
    item: &RoadmapItem,
    start_col: u32,
    end_col: u32,
    total_months: u32,
    today_col: Option<u32>,
) -> String {
    let lower_id = item.id.to_lowercase();
    let bar_class = bar_css_class(&item.status);

    // Skip if fully out of range
    if start_col >= total_months + 2 || end_col <= 2 {
        return String::new();
    }

    // Only completed items whose bar ends before today get past opacity
    let is_past = match today_col {
        Some(tc) => is_completed(&item.status) && end_col <= tc,
        None => false,
    };

    let past_class = if is_past { " past" } else { "" };

    let tooltip = format!("{} — {}", encode_attr(&item.id), encode_attr(&item.title),);

    // Linked docs in tooltip
    let linked_text = if item.linked_docs.is_empty() {
        String::new()
    } else {
        let ids: Vec<&str> = item.linked_docs.iter().map(|l| l.id.as_str()).collect();
        format!(" | Linked: {}", ids.join(", "))
    };

    let meta_parts: Vec<String> = [
        item.priority.as_ref().map(|p| format!("P:{p}")),
        item.effort.as_ref().map(|e| format!("E:{e}")),
        item.owner.as_ref().map(|o| format!("@{o}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let meta_text = if meta_parts.is_empty() {
        String::new()
    } else {
        format!(" | {}", meta_parts.join(" "))
    };

    let full_tooltip = format!("{tooltip}{meta_text}{linked_text}");

    let duration_months = end_col - start_col;
    let duration_label = if duration_months == 1 {
        "1 mo".to_string()
    } else {
        format!("{duration_months} mo")
    };

    format!(
        r#"<div class="gantt-row">
<div class="row-label"><a href="{id_href}.html" class="row-id">{id}</a><span class="row-title">{title}</span></div>
<div class="bar {bar_class}{past_class}" style="grid-column: {start_col} / {end_col}" title="{full_tooltip}"><a href="{id_href}.html">{id}</a><span class="bar-duration">{duration_label}</span></div>
</div>
"#,
        id_href = encode_attr(&lower_id),
        id = encode_text(&item.id),
        title = encode_text(&item.title),
    )
}

/// Render a row for items without a parseable date (shown as label-only).
fn render_no_bar_row(item: &RoadmapItem) -> String {
    let lower_id = item.id.to_lowercase();
    format!(
        r#"<div class="gantt-row">
<div class="row-label"><a href="{id_href}.html" class="row-id">{id}</a><span class="row-title">{title}</span></div>
<div class="bar bar-no-date" style="grid-column: 2 / 3" title="No date set">&mdash;</div>
</div>
"#,
        id_href = encode_attr(&lower_id),
        id = encode_text(&item.id),
        title = encode_text(&item.title),
    )
}

fn render_today_marker(
    today: &str,
    start_year: i32,
    start_month: u32,
    total_months: u32,
) -> String {
    let (ty, tm, td) = match parse_ymd(today) {
        Some(v) => v,
        None => return String::new(),
    };

    let idx = month_index(ty, tm, start_year, start_month);
    if idx < 0 || idx >= total_months as i32 {
        return String::new();
    }

    let dim = days_in_month(ty, tm);
    let frac = idx as f64 + td as f64 / dim as f64;
    let ratio = frac / total_months as f64;

    // The grid area for months starts after the 12rem label column.
    // Use calc() so the marker scales with the month columns only.
    format!(
        "<div class=\"today-marker\" style=\"left: calc(var(--label-w) + (100% - var(--label-w)) * {ratio:.4})\"><span class=\"today-label\">Today</span></div>\n"
    )
}

fn render_backlog_section(backlog: &[RoadmapItem]) -> String {
    let mut html = String::from("<div class=\"backlog-section\">\n");
    html.push_str(&format!(
        "<h2 class=\"backlog-title\">Backlog <span class=\"backlog-count\">{}</span></h2>\n",
        backlog.len(),
    ));
    html.push_str("<div class=\"backlog-cards\">\n");
    for item in backlog {
        html.push_str(&render_item_card(item));
    }
    html.push_str("</div>\n</div>\n");
    html
}

fn render_item_card(item: &RoadmapItem) -> String {
    let status_class = bar_css_class(&item.status);

    let mut badges = String::new();
    if let Some(ref p) = item.priority {
        badges.push_str(&format!(
            "<span class=\"badge priority-{}\">Priority: {}</span>",
            encode_attr(&p.to_lowercase()),
            encode_text(p),
        ));
    }
    if let Some(ref effort) = item.effort {
        badges.push_str(&format!(
            "<span class=\"badge badge-effort\">Effort: {}</span>",
            encode_text(effort),
        ));
    }
    for ld in &item.linked_docs {
        let lower = ld.id.to_lowercase();
        badges.push_str(&format!(
            "<a href=\"{}.html\" class=\"linked-pill {}\">{}</a>",
            encode_attr(&lower),
            encode_attr(&ld.doc_type),
            encode_text(&ld.id),
        ));
    }

    let lower_id = item.id.to_lowercase();
    format!(
        r#"<div class="item-card">
<div class="item-header">
<h3 class="item-title"><a href="{id_href}.html">{title}</a></h3>
<span class="status-badge {status_class}">{status}</span>
</div>
<div class="item-badges">{badges}</div>
</div>
"#,
        id_href = encode_attr(&lower_id),
        title = encode_text(&format!("{} — {}", item.id, item.title)),
        status = encode_text(&item.status),
    )
}

fn bar_css_class(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "pursuing" | "active" | "accepted" | "in-progress" | "in_progress" => {
            "bar-active".to_string()
        }
        "validating" | "proposed" | "draft" => "bar-proposed".to_string(),
        "completed" | "resolved" => "bar-completed".to_string(),
        "declined" | "rejected" => "bar-declined".to_string(),
        "deprecated" | "superseded" => "bar-deprecated".to_string(),
        "identified" => "bar-identified".to_string(),
        _ => format!("bar-{}", status.to_lowercase()),
    }
}

/// (light_bg, light_fg, light_border, dark_bg, dark_fg, dark_border)
const PILL_PALETTE: &[(&str, &str, &str, &str, &str, &str)] = &[
    (
        "#eff6ff",
        "#1e40af",
        "#93c5fd",
        "rgba(59,130,246,0.15)",
        "#93c5fd",
        "rgba(59,130,246,0.3)",
    ),
    (
        "#fffbeb",
        "#92400e",
        "#fcd34d",
        "rgba(245,158,11,0.15)",
        "#fcd34d",
        "rgba(245,158,11,0.3)",
    ),
    (
        "#eef2ff",
        "#3730a3",
        "#a5b4fc",
        "rgba(99,102,241,0.15)",
        "#a5b4fc",
        "rgba(99,102,241,0.3)",
    ),
    (
        "#fef2f2",
        "#991b1b",
        "#fca5a5",
        "rgba(239,68,68,0.15)",
        "#fca5a5",
        "rgba(239,68,68,0.3)",
    ),
    (
        "#ecfdf5",
        "#065f46",
        "#6ee7b7",
        "rgba(16,185,129,0.15)",
        "#6ee7b7",
        "rgba(16,185,129,0.3)",
    ),
    (
        "#faf5ff",
        "#6b21a8",
        "#c4b5fd",
        "rgba(168,85,247,0.15)",
        "#c4b5fd",
        "rgba(168,85,247,0.3)",
    ),
    (
        "#f0fdfa",
        "#115e59",
        "#5eead4",
        "rgba(20,184,166,0.15)",
        "#5eead4",
        "rgba(20,184,166,0.3)",
    ),
    (
        "#fff1f2",
        "#9f1239",
        "#fda4af",
        "rgba(244,63,94,0.15)",
        "#fda4af",
        "rgba(244,63,94,0.3)",
    ),
];

fn generate_pill_css(schema: &crate::schema::Schema) -> String {
    let nav_types = schema.nav_types();
    let mut css = String::new();

    // Light mode vars
    css.push_str("\n:root {\n");
    for (i, (key, _, _)) in nav_types.iter().enumerate() {
        let (lbg, lfg, lborder, _, _, _) = PILL_PALETTE[i % PILL_PALETTE.len()];
        css.push_str(&format!(
            "  --g-pill-{key}-bg: {lbg}; --g-pill-{key}-fg: {lfg}; --g-pill-{key}-border: {lborder};\n"
        ));
    }
    css.push_str("}\n");

    // Dark mode vars
    css.push_str(".dark {\n");
    for (i, (key, _, _)) in nav_types.iter().enumerate() {
        let (_, _, _, dbg, dfg, dborder) = PILL_PALETTE[i % PILL_PALETTE.len()];
        css.push_str(&format!(
            "  --g-pill-{key}-bg: {dbg}; --g-pill-{key}-fg: {dfg}; --g-pill-{key}-border: {dborder};\n"
        ));
    }
    css.push_str("}\n");

    // Class rules
    for (key, _, _) in &nav_types {
        css.push_str(&format!(
            ".linked-pill.{key} {{ border-color: var(--g-pill-{key}-border); background: var(--g-pill-{key}-bg); color: var(--g-pill-{key}-fg); }}\n"
        ));
    }

    css
}

const GANTT_CSS: &str = r#"
:root {
  --label-w: 20rem;
  /* Structural */
  --g-bg: #ffffff;
  --g-bg-subtle: #f8fafc;
  --g-bg-muted: #f1f5f9;
  --g-text: #0f172a;
  --g-text-secondary: #334155;
  --g-text-muted: #64748b;
  --g-text-faint: #94a3b8;
  --g-border: #e2e8f0;
  --g-border-strong: #cbd5e1;
  --g-link: #2563eb;
  --g-link-hover: #1d4ed8;
  --g-grid-line: #f1f5f9;
  --g-shadow-scroll: rgba(0,0,0,0.04);
  --g-today-label-bg: rgba(255,255,255,0.9);
  /* Current quarter */
  --g-current-bg: #f0fdf4;
  --g-current-fg: #065f46;
  /* Status bars */
  --g-bar-active: #10b981;
  --g-bar-proposed: #f59e0b;
  --g-bar-completed: #64748b;
  --g-bar-declined: #ef4444;
  --g-bar-identified: #3b82f6;
  /* Status badges */
  --g-badge-active-bg: #d1fae5; --g-badge-active-fg: #065f46;
  --g-badge-proposed-bg: #fef3c7; --g-badge-proposed-fg: #92400e;
  --g-badge-completed-bg: #e2e8f0; --g-badge-completed-fg: #334155;
  --g-badge-declined-bg: #fecdd3; --g-badge-declined-fg: #9f1239;
  --g-badge-identified-bg: #dbeafe; --g-badge-identified-fg: #1e40af;
  /* Priority badges */
  --g-pri-critical-bg: #fecdd3; --g-pri-critical-fg: #9f1239;
  --g-pri-high-bg: #fed7aa; --g-pri-high-fg: #9a3412;
  --g-pri-medium-bg: #fef3c7; --g-pri-medium-fg: #92400e;
  --g-pri-low-bg: #e0e7ff; --g-pri-low-fg: #3730a3;
  --g-effort-bg: #f3e8ff; --g-effort-fg: #6b21a8;
  /* Linked pills */
  --g-pill-bg: #f1f5f9; --g-pill-fg: #334155; --g-pill-border: #cbd5e1;
  --g-pill-hover-bg: #e2e8f0;
  /* Card hover */
  --g-card-hover-border: #93c5fd;
  --g-card-hover-shadow: rgba(37,99,235,0.12);
  /* Backlog count */
  --g-count-bg: #fef3c7; --g-count-fg: #92400e;
}

.dark {
  --g-bg: #0f172a;
  --g-bg-subtle: #1e293b;
  --g-bg-muted: #334155;
  --g-text: #e2e8f0;
  --g-text-secondary: #cbd5e1;
  --g-text-muted: #94a3b8;
  --g-text-faint: #64748b;
  --g-border: rgba(255,255,255,0.1);
  --g-border-strong: rgba(255,255,255,0.2);
  --g-link: #60a5fa;
  --g-link-hover: #93c5fd;
  --g-grid-line: rgba(255,255,255,0.06);
  --g-shadow-scroll: rgba(0,0,0,0.2);
  --g-today-label-bg: rgba(15,23,42,0.9);
  --g-current-bg: rgba(16,185,129,0.15);
  --g-current-fg: #6ee7b7;
  --g-bar-active: #059669;
  --g-bar-proposed: #d97706;
  --g-bar-completed: #475569;
  --g-bar-declined: #dc2626;
  --g-bar-identified: #2563eb;
  --g-badge-active-bg: rgba(16,185,129,0.2); --g-badge-active-fg: #6ee7b7;
  --g-badge-proposed-bg: rgba(245,158,11,0.2); --g-badge-proposed-fg: #fcd34d;
  --g-badge-completed-bg: rgba(100,116,139,0.2); --g-badge-completed-fg: #cbd5e1;
  --g-badge-declined-bg: rgba(239,68,68,0.2); --g-badge-declined-fg: #fca5a5;
  --g-badge-identified-bg: rgba(59,130,246,0.2); --g-badge-identified-fg: #93c5fd;
  --g-pri-critical-bg: rgba(239,68,68,0.2); --g-pri-critical-fg: #fca5a5;
  --g-pri-high-bg: rgba(249,115,22,0.2); --g-pri-high-fg: #fdba74;
  --g-pri-medium-bg: rgba(245,158,11,0.2); --g-pri-medium-fg: #fcd34d;
  --g-pri-low-bg: rgba(99,102,241,0.2); --g-pri-low-fg: #a5b4fc;
  --g-effort-bg: rgba(168,85,247,0.2); --g-effort-fg: #c4b5fd;
  --g-pill-bg: #1e293b; --g-pill-fg: #cbd5e1; --g-pill-border: rgba(255,255,255,0.15);
  --g-pill-hover-bg: #334155;
  --g-card-hover-border: #3b82f6;
  --g-card-hover-shadow: rgba(59,130,246,0.2);
  --g-count-bg: rgba(245,158,11,0.2); --g-count-fg: #fcd34d;
}

* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; margin: 0 auto; padding: 0 1.5rem; color: var(--g-text); line-height: 1.5; background: var(--g-bg); -webkit-font-smoothing: antialiased; }

/* Header */
header { display: flex; justify-content: space-between; align-items: center; padding: 1.5rem 0; margin-bottom: 0.5rem; border-bottom: 1px solid var(--g-border); }
.header-text h1 { font-size: 1.25rem; font-weight: 600; letter-spacing: -0.025em; color: var(--g-text); margin: 0; border: none; }
.generated { color: var(--g-text-muted); font-size: 0.75rem; margin-top: 2px; }
nav a { font-size: 0.875rem; font-weight: 500; color: var(--g-link); text-decoration: none; }
nav a:hover { color: var(--g-link-hover); text-decoration: underline; }

/* Focus styles */
a:focus-visible, .bar:focus-visible { outline: 2px solid var(--g-link); outline-offset: 2px; border-radius: 2px; }

/* Gantt scroll + body */
.gantt-scroll { overflow-x: auto; margin: 1rem 0 2rem; }
.gantt-body { position: relative; min-width: max-content; padding-top: 1.25rem; background-image: linear-gradient(to right, var(--g-grid-line) 1px, transparent 1px); background-size: calc((100% - var(--label-w)) / var(--months)) 100%; background-position: var(--label-w) 0; }

/* Grid rows: label column + N month columns */
.header-row, .gantt-row {
  display: grid;
  grid-template-columns: var(--label-w) repeat(var(--months), minmax(4rem, 1fr));
  align-items: center;
  min-height: 2.25rem;
}

/* Sticky header */
.gantt-header { position: sticky; top: 0; z-index: 3; background: var(--g-bg); border-bottom: 2px solid var(--g-border); }
.quarter-row { border-bottom: 1px solid var(--g-border); }
.corner-spacer { position: sticky; left: 0; z-index: 4; background: var(--g-bg); }
.quarter-group { text-align: center; font-weight: 600; font-size: 0.8rem; color: var(--g-text); padding: 0.5rem 0; border-left: 2px solid var(--g-border-strong); }
.quarter-group:first-of-type { border-left: none; }
.quarter-group.current { background: var(--g-current-bg); color: var(--g-current-fg); }
.month-cell { text-align: center; font-size: 0.7rem; font-weight: 500; color: var(--g-text-faint); text-transform: uppercase; padding: 0.25rem 0; border-left: 1px dashed var(--g-border); }
.month-cell.qstart { border-left: 2px solid var(--g-border-strong); }

/* Swimlane rows */
.gantt-row { border-bottom: 1px solid var(--g-grid-line); transition: background 0.1s; }
.gantt-row:hover { background: var(--g-bg-subtle); }
.gantt-row:hover .row-label { background: var(--g-bg-subtle); }
.team-group:hover .gantt-row:not(:hover) .bar { opacity: 0.5; }
.row-label {
  position: sticky;
  left: 0;
  z-index: 5;
  background: var(--g-bg);
  padding: 0.375rem 0.75rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  white-space: nowrap;
  border-right: 1px solid var(--g-border);
}
/* Scroll shadow on label column */
.row-label::after {
  content: ""; position: absolute; top: 0; right: -4px; bottom: 0; width: 4px;
  background: linear-gradient(to right, var(--g-shadow-scroll), transparent); pointer-events: none;
}
.row-id { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.7rem; font-weight: 500; color: var(--g-text-muted); flex-shrink: 0; text-decoration: none; }
.row-id:hover { color: var(--g-link); text-decoration: underline; }
.row-title { font-size: 0.8rem; font-weight: 500; color: var(--g-text-secondary); overflow: hidden; text-overflow: ellipsis; min-width: 0; }
.gantt-row:hover .row-title { color: var(--g-link); }

/* Team headers */
.team-group { border-top: 1px solid var(--g-border-strong); }
.team-header {
  background: var(--g-bg-muted);
  border-bottom: 1px solid var(--g-border);
  position: relative;
  z-index: 2;
}
.team-sticky {
  position: sticky;
  left: 0;
  display: inline-block;
  padding: 0.5rem 0.75rem;
  background: var(--g-bg-muted);
  font-weight: 700;
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--g-text-secondary);
  white-space: nowrap;
  border-right: 1px solid var(--g-border);
}

/* Bars */
.bar {
  height: 1.5rem;
  border-radius: 4px;
  display: flex;
  align-items: center;
  padding: 0 0.5rem;
  font-size: 0.7rem;
  font-weight: 600;
  color: rgba(255,255,255,0.95);
  overflow: visible;
  white-space: nowrap;
  margin: 0.25rem 2px;
  cursor: pointer;
  transition: transform 0.1s, box-shadow 0.1s, opacity 0.15s;
  box-shadow: 0 1px 2px rgba(0,0,0,0.1);
  position: relative;
}
.bar:hover { transform: translateY(-1px); box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); z-index: 20; }
.bar a { color: inherit; text-decoration: none; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
.bar a:hover { text-decoration: underline; }
.bar.past { opacity: 0.6; filter: grayscale(0.4); }
.bar-duration { margin-left: auto; font-size: 0.6rem; font-weight: 400; opacity: 0.8; flex-shrink: 0; padding-left: 0.25rem; }

/* Bar status colors */
.bar-active { background: var(--g-bar-active); }
.bar-proposed { background: var(--g-bar-proposed); }
.bar-completed { background: var(--g-bar-completed); }
.bar-declined { background: var(--g-bar-declined); }
.bar-deprecated { background: var(--g-bar-declined); }
.bar-identified { background: var(--g-bar-identified); }
.bar-no-date { background: transparent; color: var(--g-text-faint); box-shadow: none; border: 1px dashed var(--g-border-strong); }

/* Today marker */
.today-marker {
  position: absolute;
  top: 0;
  bottom: 0;
  border-left: 2px dashed #ef4444;
  z-index: 10;
  pointer-events: none;
}
.today-label {
  position: absolute;
  top: 0.25rem;
  left: 0.25rem;
  font-size: 0.6rem;
  font-weight: 700;
  color: #ef4444;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  background: var(--g-today-label-bg);
  padding: 1px 4px;
  border-radius: 3px;
}

/* Backlog section */
.backlog-section { margin: 2rem 0; }
.backlog-title { font-size: 1.125rem; font-weight: 600; letter-spacing: -0.025em; color: var(--g-text); margin-bottom: 1rem; }
.backlog-count { display: inline-block; padding: 0.125rem 0.5rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 600; background: var(--g-count-bg); color: var(--g-count-fg); vertical-align: middle; }
.backlog-cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr)); gap: 0.75rem; }

/* Item cards (backlog) */
.item-card { background: var(--g-bg); border: 1px solid var(--g-border); border-radius: 8px; padding: 1rem; box-shadow: 0 1px 3px rgba(0,0,0,0.05); transition: border-color 0.15s, box-shadow 0.15s; }
.item-card:hover { border-color: var(--g-card-hover-border); box-shadow: 0 4px 12px var(--g-card-hover-shadow); }
.item-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 0.5rem; margin-bottom: 0.5rem; }
.item-title { font-weight: 600; font-size: 0.875rem; color: var(--g-text); line-height: 1.3; }
.item-title a { color: inherit; text-decoration: none; }
.item-title a:hover { text-decoration: underline; }

/* Status badges (backlog cards) */
.status-badge { display: inline-block; padding: 0.25rem 0.625rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 500; white-space: nowrap; flex-shrink: 0; }
.bar-active.status-badge, .status-badge.bar-active { background: var(--g-badge-active-bg); color: var(--g-badge-active-fg); }
.bar-proposed.status-badge, .status-badge.bar-proposed { background: var(--g-badge-proposed-bg); color: var(--g-badge-proposed-fg); }
.bar-completed.status-badge, .status-badge.bar-completed { background: var(--g-badge-completed-bg); color: var(--g-badge-completed-fg); }
.bar-declined.status-badge, .status-badge.bar-declined { background: var(--g-badge-declined-bg); color: var(--g-badge-declined-fg); }
.bar-deprecated.status-badge, .status-badge.bar-deprecated { background: var(--g-badge-declined-bg); color: var(--g-badge-declined-fg); }
.bar-identified.status-badge, .status-badge.bar-identified { background: var(--g-badge-identified-bg); color: var(--g-badge-identified-fg); }

/* Metadata badges */
.item-badges { display: flex; flex-wrap: wrap; gap: 0.375rem; }
.badge { display: inline-block; padding: 0.25rem 0.625rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 500; }
.priority-critical { background: var(--g-pri-critical-bg); color: var(--g-pri-critical-fg); }
.priority-high { background: var(--g-pri-high-bg); color: var(--g-pri-high-fg); }
.priority-medium { background: var(--g-pri-medium-bg); color: var(--g-pri-medium-fg); }
.priority-low { background: var(--g-pri-low-bg); color: var(--g-pri-low-fg); }
.badge-effort { background: var(--g-effort-bg); color: var(--g-effort-fg); }

/* Linked doc pills */
.linked-pill { display: inline-block; padding: 0.25rem 0.625rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 500; background: var(--g-pill-bg); color: var(--g-pill-fg); border: 1px solid var(--g-pill-border); text-decoration: none; }
.linked-pill:hover { background: var(--g-pill-hover-bg); text-decoration: none; }

/* Responsive */
@media (max-width: 640px) {
  :root { --label-w: 14rem; }
  .header-row, .gantt-row { grid-template-columns: var(--label-w) repeat(var(--months), minmax(3rem, 1fr)); }
  header { flex-direction: column; align-items: flex-start; gap: 0.5rem; }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quarter_from_date() {
        assert_eq!(
            Quarter::from_date("2026-01-15"),
            Some(Quarter::new(2026, 1))
        );
        assert_eq!(
            Quarter::from_date("2026-06-30"),
            Some(Quarter::new(2026, 2))
        );
        assert_eq!(
            Quarter::from_date("2025-12-01"),
            Some(Quarter::new(2025, 4))
        );
        assert_eq!(Quarter::from_date("invalid"), None);
    }

    #[test]
    fn test_quarter_parse() {
        assert_eq!(Quarter::parse("2026-Q1"), Some(Quarter::new(2026, 1)));
        assert_eq!(Quarter::parse("2025-Q4"), Some(Quarter::new(2025, 4)));
        assert_eq!(Quarter::parse("2026-Q5"), None);
        assert_eq!(Quarter::parse("invalid"), None);
    }

    #[test]
    fn test_quarter_offset() {
        let q = Quarter::new(2026, 1);
        assert_eq!(q.offset(1), Quarter::new(2026, 2));
        assert_eq!(q.offset(4), Quarter::new(2027, 1));
        assert_eq!(q.offset(-1), Quarter::new(2025, 4));
        assert_eq!(q.offset(-4), Quarter::new(2025, 1));
    }

    #[test]
    fn test_quarter_label() {
        assert_eq!(Quarter::new(2026, 1).label(), "Q1 2026");
        assert_eq!(Quarter::new(2025, 4).label(), "Q4 2025");
    }

    #[test]
    fn test_quarter_ordering() {
        assert!(Quarter::new(2025, 4) < Quarter::new(2026, 1));
        assert!(Quarter::new(2026, 1) < Quarter::new(2026, 2));
    }

    #[test]
    fn test_capacity_add_effort() {
        let mut slots = CapacitySlots::default();
        slots.add_effort("large");
        slots.add_effort("medium");
        slots.add_effort("small");
        slots.add_effort("l");
        slots.add_effort("unknown");
        assert_eq!(slots.large, 2);
        assert_eq!(slots.medium, 2); // 1 explicit + 1 unknown default
        assert_eq!(slots.small, 1);
    }

    #[test]
    fn test_priority_rank() {
        assert!(priority_rank(Some("critical")) < priority_rank(Some("high")));
        assert!(priority_rank(Some("high")) < priority_rank(Some("medium")));
        assert!(priority_rank(Some("medium")) < priority_rank(Some("low")));
        assert!(priority_rank(Some("low")) < priority_rank(None));
    }

    #[test]
    fn test_status_categories() {
        assert!(is_completed("completed"));
        assert!(is_completed("Resolved"));
        assert!(is_completed("DECLINED"));
        assert!(!is_completed("pursuing"));

        assert!(is_active("pursuing"));
        assert!(is_active("Validating"));
        assert!(!is_active("identified"));

        assert!(is_backlog("identified"));
        assert!(is_backlog("Proposed"));
        assert!(!is_backlog("pursuing"));
    }

    #[test]
    fn test_bar_css_class() {
        assert_eq!(bar_css_class("pursuing"), "bar-active");
        assert_eq!(bar_css_class("Validating"), "bar-proposed");
        assert_eq!(bar_css_class("completed"), "bar-completed");
        assert_eq!(bar_css_class("declined"), "bar-declined");
        assert_eq!(bar_css_class("identified"), "bar-identified");
    }

    #[test]
    fn test_parse_ymd() {
        assert_eq!(parse_ymd("2026-01-15"), Some((2026, 1, 15)));
        assert_eq!(parse_ymd("2025-12-31"), Some((2025, 12, 31)));
        assert_eq!(parse_ymd("invalid"), None);
        assert_eq!(parse_ymd("2026-13-01"), None);
        assert_eq!(parse_ymd("2026-00-01"), None);
    }

    #[test]
    fn test_effort_to_months() {
        assert_eq!(effort_to_months(Some("small")), 1);
        assert_eq!(effort_to_months(Some("s")), 1);
        assert_eq!(effort_to_months(Some("medium")), 3);
        assert_eq!(effort_to_months(Some("large")), 6);
        assert_eq!(effort_to_months(Some("l")), 6);
        assert_eq!(effort_to_months(None), 3);
        assert_eq!(effort_to_months(Some("unknown")), 3);
    }

    #[test]
    fn test_month_index() {
        assert_eq!(month_index(2026, 1, 2026, 1), 0);
        assert_eq!(month_index(2026, 3, 2026, 1), 2);
        assert_eq!(month_index(2026, 1, 2025, 10), 3);
        assert_eq!(month_index(2025, 10, 2026, 1), -3);
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // leap
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2000, 2), 29); // century leap
        assert_eq!(days_in_month(1900, 2), 28); // century non-leap
    }

    #[test]
    fn test_quarter_first_last_month() {
        let q1 = Quarter::new(2026, 1);
        assert_eq!(q1.first_month(), 1);
        assert_eq!(q1.last_month(), 3);

        let q4 = Quarter::new(2026, 4);
        assert_eq!(q4.first_month(), 10);
        assert_eq!(q4.last_month(), 12);
    }

    #[test]
    fn test_roadmap_config_parse() {
        let yaml = r#"
capacity:
  default: { large: 1, medium: 2, small: 3 }
  teams:
    platform: { large: 1, medium: 2, small: 2 }
    security: { large: 0, medium: 1, small: 2 }
  overrides:
    "2026-Q1":
      platform: { large: 0, medium: 1, small: 1, note: "Team offsite" }
display:
  past_quarters: 2
  future_quarters: 3
"#;
        let config: RoadmapConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.display.past_quarters, 2);
        assert_eq!(config.display.future_quarters, 3);

        let default = config.capacity.default.clone().unwrap();
        assert_eq!(default.large, 1);
        assert_eq!(default.medium, 2);

        let platform = &config.capacity.teams["platform"];
        assert_eq!(platform.large, 1);
        assert_eq!(platform.small, 2);

        // Override
        let q1_override = &config.capacity.overrides["2026-Q1"]["platform"];
        assert_eq!(q1_override.large, 0);
        assert_eq!(q1_override.note.as_deref(), Some("Team offsite"));

        // team_capacity resolution
        let q1 = Quarter::new(2026, 1);
        let q2 = Quarter::new(2026, 2);

        let platform_q1 = config.team_capacity("platform", &q1);
        assert_eq!(platform_q1.large, 0); // override
        assert_eq!(platform_q1.note.as_deref(), Some("Team offsite"));

        let platform_q2 = config.team_capacity("platform", &q2);
        assert_eq!(platform_q2.large, 1); // team default

        let unknown_q1 = config.team_capacity("unknown-team", &q1);
        assert_eq!(unknown_q1.large, 1); // global default
    }
}
