//! Integration tests using the fixture documents.
//!
//! Fixture graph (cross-doc relations):
//!
//!   ADR-001 ──enables──> OPP-001
//!   ADR-001 ──triggers──> POL-001
//!   ADR-001 ──related──> ADR-002
//!
//!   ADR-002 ──enabled_by──> ADR-001
//!   ADR-002 ──related──> ADR-001, OPP-001
//!
//!   ADR-003 ──superseded_by──> ADR-005 (unresolved)
//!   ADR-003 ──caused_by──> INC-001
//!
//!   OPP-001 ──enabled_by──> ADR-001, ADR-002
//!   OPP-001 ──triggers──> ADR-003
//!   OPP-001 ──blocked_by──> POL-001
//!   OPP-001 ──related──> INC-001
//!
//!   POL-001 ──caused_by──> ADR-001
//!   POL-001 ──blocks──> OPP-001
//!   POL-001 ──enables──> ADR-002
//!
//!   INC-001 ──caused_by──> ADR-001
//!   INC-001 ──triggers──> ADR-003
//!   INC-001 ──enables──> OPP-001
//!   INC-001 ──related──> POL-001

use std::collections::HashSet;
use std::path::PathBuf;

use md_db::discovery::{self, Filter};
use md_db::document::Document;
use md_db::schema::Schema;
use md_db::users::OrgConfig;
use md_db::validation;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

fn load_schema() -> Schema {
    Schema::from_file(fixture("schema.kdl")).unwrap()
}

fn load_users() -> OrgConfig {
    OrgConfig::from_file(fixture("org.kdl")).unwrap()
}

fn load_doc(name: &str) -> Document {
    Document::from_file(fixture(name)).unwrap()
}

// ─── Document loading ────────────────────────────────────────────────────────

#[test]
fn load_all_fixture_docs() {
    for name in &[
        "adr-001-use-postgresql.md",
        "adr-002-use-rest-api.md",
        "adr-003-use-redis-for-caching.md",
        "opp-001-real-time-collaboration.md",
        "pol-001-data-retention-policy.md",
        "inc-001-database-connection-pool-exhaustion.md",
    ] {
        let doc = load_doc(name);
        assert!(doc.frontmatter.is_some(), "{name} missing frontmatter");
    }
}

// ─── Frontmatter field reads ─────────────────────────────────────────────────

#[test]
fn adr_001_fields() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(doc.title().unwrap(), "Use PostgreSQL");
    assert_eq!(fm.get_display("status").unwrap(), "accepted");
    assert_eq!(fm.get_display("author").unwrap(), "onni");
    assert_eq!(fm.get_display("date").unwrap(), "2025-01-10");
}

#[test]
fn inc_001_fields() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.get_display("severity").unwrap(), "sev2");
    assert_eq!(fm.get_display("commander").unwrap(), "onni");
    assert_eq!(fm.get_display("customer_impact").unwrap(), "degraded");

    // number field
    let duration = fm.get("duration_minutes").unwrap();
    assert_eq!(duration.as_i64(), Some(93));
}

#[test]
fn opp_001_fields() {
    let doc = load_doc("opp-001-real-time-collaboration.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.get_display("priority").unwrap(), "high");
    assert_eq!(fm.get_display("effort").unwrap(), "large");
    assert_eq!(fm.get_display("impact").unwrap(), "xl");
}

#[test]
fn pol_001_fields() {
    let doc = load_doc("pol-001-data-retention-policy.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.get_display("category").unwrap(), "policy");
    assert_eq!(fm.get_display("approver").unwrap(), "cto");
    assert!(fm.has_field("audience"));
    assert!(fm.has_field("tags"));
}

// ─── Relation fields (cross-doc links in frontmatter) ────────────────────────

#[test]
fn adr_001_enables_opp_001() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let fm = doc.frontmatter().unwrap();
    let enables = fm.get("enables").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = enables.iter().filter_map(|v| v.as_str()).collect();
    assert!(refs.contains(&"OPP-001"));
}

#[test]
fn adr_001_triggers_pol_001() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let fm = doc.frontmatter().unwrap();
    let triggers = fm.get("triggers").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = triggers.iter().filter_map(|v| v.as_str()).collect();
    assert!(refs.contains(&"POL-001"));
}

#[test]
fn adr_001_related_to_adr_002() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let fm = doc.frontmatter().unwrap();
    let related = fm.get("related").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = related.iter().filter_map(|v| v.as_str()).collect();
    assert!(refs.contains(&"ADR-002"));
}

#[test]
fn adr_002_inverse_enabled_by_adr_001() {
    let doc = load_doc("adr-002-use-rest-api.md");
    let fm = doc.frontmatter().unwrap();
    let enabled_by = fm.get("enabled_by").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = enabled_by.iter().filter_map(|v| v.as_str()).collect();
    assert!(refs.contains(&"ADR-001"));
}

#[test]
fn adr_002_related_to_multiple() {
    let doc = load_doc("adr-002-use-rest-api.md");
    let fm = doc.frontmatter().unwrap();
    let related = fm.get("related").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = related.iter().filter_map(|v| v.as_str()).collect();
    assert!(refs.contains(&"ADR-001"));
    assert!(refs.contains(&"OPP-001"));
}

#[test]
fn adr_003_superseded_by_unresolved() {
    let doc = load_doc("adr-003-use-redis-for-caching.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.get_display("superseded_by").unwrap(), "ADR-005");
}

#[test]
fn adr_003_caused_by_inc_001() {
    let doc = load_doc("adr-003-use-redis-for-caching.md");
    let fm = doc.frontmatter().unwrap();
    assert_eq!(fm.get_display("caused_by").unwrap(), "INC-001");
}

#[test]
fn opp_001_complex_relations() {
    let doc = load_doc("opp-001-real-time-collaboration.md");
    let fm = doc.frontmatter().unwrap();

    // enabled_by: ADR-001, ADR-002
    let enabled_by = fm.get("enabled_by").unwrap().as_sequence().unwrap();
    let refs: Vec<&str> = enabled_by.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(refs, vec!["ADR-001", "ADR-002"]);

    // triggers: ADR-003
    let triggers = fm.get("triggers").unwrap().as_sequence().unwrap();
    assert_eq!(triggers[0].as_str().unwrap(), "ADR-003");

    // blocked_by: POL-001
    let blocked_by = fm.get("blocked_by").unwrap().as_sequence().unwrap();
    assert_eq!(blocked_by[0].as_str().unwrap(), "POL-001");

    // related: INC-001
    let related = fm.get("related").unwrap().as_sequence().unwrap();
    assert_eq!(related[0].as_str().unwrap(), "INC-001");
}

#[test]
fn pol_001_bidirectional_relations() {
    let doc = load_doc("pol-001-data-retention-policy.md");
    let fm = doc.frontmatter().unwrap();

    // caused_by (inverse of triggers): ADR-001
    assert_eq!(fm.get_display("caused_by").unwrap(), "ADR-001");

    // blocks: OPP-001
    let blocks = fm.get("blocks").unwrap().as_sequence().unwrap();
    assert_eq!(blocks[0].as_str().unwrap(), "OPP-001");

    // enables: ADR-002
    let enables = fm.get("enables").unwrap().as_sequence().unwrap();
    assert_eq!(enables[0].as_str().unwrap(), "ADR-002");
}

#[test]
fn inc_001_all_relations() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let fm = doc.frontmatter().unwrap();

    assert_eq!(fm.get_display("caused_by").unwrap(), "ADR-001");

    let triggers = fm.get("triggers").unwrap().as_sequence().unwrap();
    assert_eq!(triggers[0].as_str().unwrap(), "ADR-003");

    let enables = fm.get("enables").unwrap().as_sequence().unwrap();
    assert_eq!(enables[0].as_str().unwrap(), "OPP-001");

    let related = fm.get("related").unwrap().as_sequence().unwrap();
    assert_eq!(related[0].as_str().unwrap(), "POL-001");
}

// ─── Section extraction ──────────────────────────────────────────────────────

#[test]
fn adr_001_decision_section() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let section = doc.get_section("Decision").unwrap();
    assert!(section.content.contains("PostgreSQL"));
    assert!(section.content.contains("Rationale"));
}

#[test]
fn adr_001_nested_consequences() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let positive = doc
        .get_section_by_path(&["Consequences", "Positive"])
        .unwrap();
    assert!(positive.content.contains("ACID"));

    let negative = doc
        .get_section_by_path(&["Consequences", "Negative"])
        .unwrap();
    assert!(negative.content.contains("complexity"));
}

#[test]
fn inc_001_all_required_sections() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    for name in &[
        "Summary",
        "Impact",
        "Timeline",
        "Root Cause",
        "Resolution",
        "Action Items",
        "Lessons Learned",
    ] {
        assert!(
            doc.get_section(name).is_ok(),
            "INC-001 missing section: {name}"
        );
    }
}

#[test]
fn opp_001_required_sections() {
    let doc = load_doc("opp-001-real-time-collaboration.md");
    for name in &["Problem", "Proposed Solution", "Success Criteria"] {
        assert!(
            doc.get_section(name).is_ok(),
            "OPP-001 missing section: {name}"
        );
    }
}

#[test]
fn pol_001_nested_compliance() {
    let doc = load_doc("pol-001-data-retention-policy.md");
    let requirements = doc
        .get_section_by_path(&["Compliance", "Requirements"])
        .unwrap();
    assert!(requirements.content.contains("automated deletion"));

    let exceptions = doc
        .get_section_by_path(&["Compliance", "Exceptions"])
        .unwrap();
    assert!(exceptions.content.contains("legal"));
}

#[test]
fn section_plain_text() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let summary = doc.get_section("Summary").unwrap();
    let text = summary.text();
    assert!(text.contains("connection pool exhausted"));
    assert!(!text.contains("#")); // no markdown syntax
}

// ─── Table extraction ────────────────────────────────────────────────────────

#[test]
fn adr_001_alternatives_table() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let section = doc.get_section("Alternatives Considered").unwrap();
    let tables = section.tables();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.headers(), &["Option", "Score", "Notes"]);
    assert_eq!(table.rows().len(), 3);

    assert_eq!(table.get_cell("Option", 0), Some("PostgreSQL"));
    assert_eq!(table.get_cell("Score", 0), Some("9"));
    assert_eq!(table.get_cell("Score", 1), Some("7"));
    assert_eq!(
        table.get_cell("Notes", 2),
        Some("Not suitable for production")
    );
}

#[test]
fn adr_001_table_json() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let section = doc.get_section("Alternatives Considered").unwrap();
    let table = &section.tables()[0];
    let json = table.to_json();

    assert_eq!(json[0]["Option"], "PostgreSQL");
    assert_eq!(json[0]["Score"], "9");
    assert_eq!(json[1]["Option"], "MySQL");
    assert_eq!(json[2]["Option"], "SQLite");
}

#[test]
fn adr_001_table_column() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let section = doc.get_section("Alternatives Considered").unwrap();
    let table = &section.tables()[0];

    let scores = table.get_column("Score").unwrap();
    assert_eq!(scores, vec!["9", "7", "5"]);

    let options = table.get_column("Option").unwrap();
    assert_eq!(options, vec!["PostgreSQL", "MySQL", "SQLite"]);
}

#[test]
fn inc_001_timeline_table() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let section = doc.get_section("Timeline").unwrap();
    let tables = section.tables();
    assert_eq!(tables.len(), 1);

    let table = &tables[0];
    assert_eq!(table.headers(), &["Time", "Event", "Actor"]);
    assert!(table.rows().len() >= 7);
    assert_eq!(table.get_cell("Time", 0), Some("14:32"));
    assert_eq!(table.get_cell("Actor", 1), Some("@onni"));
}

#[test]
fn inc_001_action_items_table() {
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let section = doc.get_section("Action Items").unwrap();
    let table = &section.tables()[0];

    assert_eq!(table.headers(), &["Action", "Owner", "Due Date", "Status"]);
    assert_eq!(table.rows().len(), 4);

    // Check owners are user refs
    let owners = table.get_column("Owner").unwrap();
    assert_eq!(owners, vec!["@alice", "@bob", "@onni", "@alice"]);

    // Check statuses
    let statuses = table.get_column("Status").unwrap();
    assert!(statuses.contains(&"done"));
    assert!(statuses.contains(&"in-progress"));
    assert!(statuses.contains(&"pending"));
}

#[test]
fn opp_001_risks_table() {
    let doc = load_doc("opp-001-real-time-collaboration.md");
    let section = doc.get_section("Risks").unwrap();
    let table = &section.tables()[0];

    assert_eq!(
        table.headers(),
        &["Risk", "Likelihood", "Impact", "Mitigation"]
    );
    assert_eq!(table.rows().len(), 3);
    assert_eq!(table.get_cell("Risk", 0), Some("CRDT complexity"));
}

#[test]
fn pol_001_roles_table() {
    let doc = load_doc("pol-001-data-retention-policy.md");
    let section = doc.get_section("Roles and Responsibilities").unwrap();
    let table = &section.tables()[0];

    assert_eq!(table.headers(), &["Role", "Responsibility"]);
    assert_eq!(table.rows().len(), 4);
    assert_eq!(table.get_cell("Role", 0), Some("Data Owner"));
}

// ─── Full document JSON ──────────────────────────────────────────────────────

#[test]
fn adr_001_to_json() {
    let doc = load_doc("adr-001-use-postgresql.md");
    let json = doc.to_json();
    assert_eq!(json["frontmatter"]["status"], "accepted");
    assert_eq!(json["frontmatter"]["author"], "onni");
    // Top-level section is the H1 title; H2s are subsections under it
    assert!(!json["sections"].as_array().unwrap().is_empty());
}

// ─── Schema parsing ─────────────────────────────────────────────────────────

#[test]
fn schema_loads_all_types() {
    let schema = load_schema();
    assert!(schema.get_type("adr").is_some());
    assert!(schema.get_type("opp").is_some());
    assert!(schema.get_type("pol").is_some());
    assert!(schema.get_type("inc").is_some());
    assert!(schema.get_type("nonexistent").is_none());
}

#[test]
fn schema_relations() {
    let schema = load_schema();
    assert_eq!(schema.relations.len(), 5);

    let names = schema.all_relation_field_names();
    assert!(names.contains(&"supersedes"));
    assert!(names.contains(&"superseded_by"));
    assert!(names.contains(&"enables"));
    assert!(names.contains(&"enabled_by"));
    assert!(names.contains(&"triggers"));
    assert!(names.contains(&"caused_by"));
    assert!(names.contains(&"blocks"));
    assert!(names.contains(&"blocked_by"));
    assert!(names.contains(&"related"));
}

#[test]
fn schema_find_relation_by_inverse() {
    let schema = load_schema();

    let (rel, is_inv) = schema.find_relation("caused_by").unwrap();
    assert_eq!(rel.name, "triggers");
    assert!(is_inv);

    let (rel, is_inv) = schema.find_relation("superseded_by").unwrap();
    assert_eq!(rel.name, "supersedes");
    assert!(is_inv);

    let (rel, is_inv) = schema.find_relation("related").unwrap();
    assert_eq!(rel.name, "related");
    assert!(!is_inv);
}

#[test]
fn schema_relation_cardinality() {
    let schema = load_schema();

    assert_eq!(
        schema.relation_cardinality("supersedes"),
        Some(md_db::schema::Cardinality::One)
    );
    assert_eq!(
        schema.relation_cardinality("superseded_by"),
        Some(md_db::schema::Cardinality::One)
    );
    assert_eq!(
        schema.relation_cardinality("enables"),
        Some(md_db::schema::Cardinality::Many)
    );
    assert_eq!(
        schema.relation_cardinality("related"),
        Some(md_db::schema::Cardinality::Many)
    );
}

#[test]
fn schema_adr_type_def() {
    let schema = load_schema();
    let adr = schema.get_type("adr").unwrap();

    // Fields
    let field_names: Vec<&str> = adr.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"status"));
    assert!(field_names.contains(&"author"));
    assert!(field_names.contains(&"date"));
    assert!(field_names.contains(&"reviewers"));
    assert!(field_names.contains(&"tags"));

    // author is type=user
    let author = adr.fields.iter().find(|f| f.name == "author").unwrap();
    assert_eq!(author.field_type, md_db::schema::FieldType::User);
    assert!(author.required);

    // reviewers is type=user[]
    let reviewers = adr.fields.iter().find(|f| f.name == "reviewers").unwrap();
    assert_eq!(reviewers.field_type, md_db::schema::FieldType::UserArray);

    // Sections
    let sec_names: Vec<&str> = adr.sections.iter().map(|s| s.name.as_str()).collect();
    assert!(sec_names.contains(&"Decision"));
    assert!(sec_names.contains(&"Consequences"));
    assert!(sec_names.contains(&"Alternatives Considered"));
}

#[test]
fn schema_inc_action_items_user_column() {
    let schema = load_schema();
    let inc = schema.get_type("inc").unwrap();

    let action_items = inc
        .sections
        .iter()
        .find(|s| s.name == "Action Items")
        .unwrap();
    let table = action_items.table.as_ref().unwrap();
    let owner_col = table.columns.iter().find(|c| c.name == "Owner").unwrap();
    assert_eq!(owner_col.col_type, md_db::schema::FieldType::User);
    assert!(owner_col.required);
}

// ─── User/team config ────────────────────────────────────────────────────────

#[test]
fn users_config_loads() {
    let uc = load_users();
    assert!(uc.users.contains_key("onni"));
    assert!(uc.users.contains_key("alice"));
    assert!(uc.users.contains_key("bob"));
    assert!(uc.users.contains_key("cto"));
    assert!(uc.teams.contains_key("platform"));
    assert!(uc.teams.contains_key("security"));
    assert!(uc.teams.contains_key("engineering"));
}

#[test]
fn users_valid_refs() {
    let uc = load_users();
    assert!(uc.is_valid_ref("@onni"));
    assert!(uc.is_valid_ref("@alice"));
    assert!(uc.is_valid_ref("@bob"));
    assert!(uc.is_valid_ref("@cto"));
    assert!(uc.is_valid_ref("@team/platform"));
    assert!(uc.is_valid_ref("@team/engineering"));
    assert!(!uc.is_valid_ref("@nonexistent"));
    assert!(!uc.is_valid_ref("@team/nonexistent"));
}

#[test]
fn users_nested_team_expansion() {
    let uc = load_users();

    // Engineering = platform + security
    // platform members: onni, alice
    // security members: bob
    let eng = uc.expand_team_members("engineering");
    assert!(eng.contains("onni"));
    assert!(eng.contains("alice"));
    assert!(eng.contains("bob"));
    assert_eq!(eng.len(), 3);
}

#[test]
fn users_extra_attributes() {
    let uc = load_users();
    let onni = &uc.users["onni"];
    assert_eq!(onni.extra["role"], "staff-engineer");
    assert_eq!(onni.title.as_deref(), Some("Staff Engineer"));

    let platform = &uc.teams["platform"];
    assert_eq!(platform.lead.as_deref(), Some("onni"));
}

// ─── Validation: all fixtures pass ───────────────────────────────────────────

#[test]
fn validate_all_fixtures_pass() {
    let schema = load_schema();
    let uc = load_users();
    let result = validation::validate_directory(fixtures_dir(), &schema, None, Some(&uc)).unwrap();

    // Only expected warning: ADR-003 refs ADR-005 which doesn't exist
    assert_eq!(
        result.total_errors(),
        0,
        "Unexpected errors:\n{}",
        result.to_report()
    );
    assert_eq!(result.total_warnings(), 1); // ADR-005 unresolved
}

#[test]
fn validate_all_fixtures_without_users_still_passes() {
    let schema = load_schema();
    let result = validation::validate_directory(fixtures_dir(), &schema, None, None).unwrap();

    assert_eq!(
        result.total_errors(),
        0,
        "Unexpected errors:\n{}",
        result.to_report()
    );
}

#[test]
fn validate_adr_005_unresolved_is_warning_not_error() {
    let schema = load_schema();
    let result = validation::validate_directory(fixtures_dir(), &schema, None, None).unwrap();

    let adr003 = result
        .file_results
        .iter()
        .find(|f| f.path.contains("adr-003"))
        .unwrap();

    assert_eq!(adr003.warnings(), 1);
    assert!(adr003.diagnostics[0].message.contains("ADR-005"));
    assert_eq!(adr003.diagnostics[0].code, "R011");
}

// ─── Validation: individual document against schema ──────────────────────────

#[test]
fn validate_adr_001_individual() {
    let schema = load_schema();
    let uc = load_users();
    let doc = load_doc("adr-001-use-postgresql.md");

    let known_ids: HashSet<String> = [
        "ADR-001", "ADR-002", "ADR-003", "OPP-001", "POL-001", "INC-001",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &known_ids, Some(&uc));
    assert_eq!(
        result.errors(),
        0,
        "ADR-001 errors: {:?}",
        result.diagnostics
    );
}

#[test]
fn validate_inc_001_individual() {
    let schema = load_schema();
    let uc = load_users();
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");

    let known_ids: HashSet<String> = ["ADR-001", "ADR-003", "OPP-001", "POL-001", "INC-001"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &known_ids, Some(&uc));
    assert_eq!(
        result.errors(),
        0,
        "INC-001 errors: {:?}",
        result.diagnostics
    );
}

// ─── Validation: detecting errors ────────────────────────────────────────────

#[test]
fn validate_missing_required_fields() {
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: Incomplete\n---\n\n# Decision\n\nX\n\n# Consequences\n\n## Positive\n\nY\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
    // Missing: status, author, date
    assert!(
        result.errors() >= 3,
        "expected >=3 errors, got: {:?}",
        result.diagnostics
    );
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "F010" && d.message.contains("status")));
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "F010" && d.message.contains("author")));
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "F010" && d.message.contains("date")));
}

#[test]
fn validate_missing_required_sections() {
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: onni\ndate: \"2025-01-01\"\n---\n\n# Decision\n\nDone.\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
    // Missing: Consequences > Positive
    assert!(result.diagnostics.iter().any(|d| d.code == "S010"));
}

#[test]
fn validate_bad_enum_value() {
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: inc\ntitle: T\nstatus: banana\nseverity: sev1\ncommander: onni\nstarted_at: \"2025-01-01T00:00:00Z\"\naffected_systems:\n  - api\ncustomer_impact: none\n---\n\n# Summary\nX\n# Impact\nX\n# Timeline\n\n| Time | Event | Actor |\n|---|---|---|\n| 0 | E | A |\n\n# Root Cause\nX\n# Resolution\nX\n# Action Items\n\n| Action | Owner | Due | Status |\n|---|---|---|---|\n| A | @onni | - | - |\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "F021" && d.message.contains("banana")));
}

#[test]
fn validate_bad_date_pattern() {
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: onni\ndate: not-a-date\n---\n\n# Decision\nX\n# Consequences\n## Positive\nY\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "F030" && d.message.contains("date")));
}

#[test]
fn validate_unknown_user_ref() {
    let schema = load_schema();
    let uc = load_users();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: ghost\ndate: \"2025-01-01\"\n---\n\n# Decision\nX\n# Consequences\n## Positive\nY\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
    assert!(result
        .diagnostics
        .iter()
        .any(|d| d.code == "U011" && d.message.contains("ghost")));
}

#[test]
fn validate_user_without_at_prefix() {
    // Bare handles (no @) are valid in frontmatter — schema knows the field type
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: onni\ndate: \"2025-01-01\"\n---\n\n# Decision\nX\n# Consequences\n## Positive\nY\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "U010"),
        "bare handles in frontmatter should not trigger U010: {:?}",
        result.diagnostics
    );
}

#[test]
fn validate_team_ref_in_user_field() {
    let schema = load_schema();
    let uc = load_users();
    // Using team/platform as author — valid ref (no @ in frontmatter)
    let doc = Document::from_str(
        "---\ntype: adr\nstatus: accepted\nauthor: team/platform\ndate: \"2025-01-01\"\n---\n\n# T\n\nIntro.\n\n## Decision\nX\n## Consequences\n### Positive\nY\n",
    )
    .unwrap();

    let result =
        validation::validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
    assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
}

#[test]
fn validate_broken_relation_ref() {
    let schema = load_schema();
    let doc = Document::from_str(
        "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: onni\ndate: \"2025-01-01\"\nenables:\n  - \"NONEXISTENT-999\"\n---\n\n# Decision\nX\n# Consequences\n## Positive\nY\n",
    )
    .unwrap();

    let known_ids: HashSet<String> = ["ADR-001"].iter().map(|s| s.to_string()).collect();
    let result = validation::validate_document(&doc, &schema, &HashSet::new(), &known_ids, None);
    // NONEXISTENT-999 doesn't match ref-format patterns
    assert!(result.diagnostics.iter().any(|d| d.code == "R001"));
}

// ─── Discovery with filters ─────────────────────────────────────────────────

#[test]
fn discover_all_md_files() {
    let files = discovery::discover_files(fixtures_dir(), None, &[], false).unwrap();
    assert!(files.len() >= 6); // at least our 6 fixture .md files
}

#[test]
fn discover_adr_pattern() {
    let files = discovery::discover_files(fixtures_dir(), Some("adr-*.md"), &[], false).unwrap();
    assert_eq!(files.len(), 3);
    assert!(files
        .iter()
        .all(|f| f.file_name().unwrap().to_str().unwrap().starts_with("adr-")));
}

#[test]
fn discover_by_status_filter() {
    let files = discovery::discover_files(
        fixtures_dir(),
        None,
        &[Filter::FieldEquals {
            key: "status".into(),
            value: "accepted".into(),
        }],
        false,
    )
    .unwrap();
    // Only ADR-001 has status=accepted
    assert_eq!(files.len(), 1);
    assert!(files[0].to_str().unwrap().contains("adr-001"));
}

#[test]
fn discover_by_type_filter() {
    // Type is inferred from filename, not frontmatter — filter by severity which is inc-only
    let files = discovery::discover_files(
        fixtures_dir(),
        None,
        &[Filter::HasField("severity".into())],
        false,
    )
    .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].to_str().unwrap().contains("inc-001"));
}

#[test]
fn discover_has_field_filter() {
    let files = discovery::discover_files(
        fixtures_dir(),
        None,
        &[Filter::HasField("superseded_by".into())],
        false,
    )
    .unwrap();
    // Only ADR-003 has superseded_by
    assert_eq!(files.len(), 1);
    assert!(files[0].to_str().unwrap().contains("adr-003"));
}

#[test]
fn discover_has_field_severity() {
    let files = discovery::discover_files(
        fixtures_dir(),
        None,
        &[Filter::HasField("severity".into())],
        false,
    )
    .unwrap();
    // Only INC-001 has severity
    assert_eq!(files.len(), 1);
}

#[test]
fn discover_combined_filters() {
    // Filter by status=accepted and related field (only ADR-001 matches)
    let files = discovery::discover_files(
        fixtures_dir(),
        None,
        &[
            Filter::FieldEquals {
                key: "status".into(),
                value: "accepted".into(),
            },
            Filter::HasField("related".into()),
        ],
        false,
    )
    .unwrap();
    // Only ADR-001 has both status=accepted and a related field
    assert_eq!(files.len(), 1);
}

// ─── Querying linked docs ────────────────────────────────────────────────────

/// Given a document, collect all doc IDs it references via relation fields.
fn collect_outgoing_refs(doc: &Document, schema: &Schema) -> Vec<String> {
    let fm = match doc.frontmatter.as_ref() {
        Some(fm) => fm,
        None => return Vec::new(),
    };
    let mut refs = Vec::new();
    for key in fm.keys() {
        if schema.find_relation(key).is_some() {
            if let Some(val) = fm.get(key) {
                if let Some(s) = val.as_str() {
                    refs.push(s.to_string());
                }
                if let Some(seq) = val.as_sequence() {
                    for item in seq {
                        if let Some(s) = item.as_str() {
                            refs.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
    refs
}

#[test]
fn query_outgoing_refs_from_adr_001() {
    let schema = load_schema();
    let doc = load_doc("adr-001-use-postgresql.md");
    let refs = collect_outgoing_refs(&doc, &schema);
    assert!(refs.contains(&"OPP-001".to_string()));
    assert!(refs.contains(&"POL-001".to_string()));
    assert!(refs.contains(&"ADR-002".to_string()));
}

#[test]
fn query_outgoing_refs_from_inc_001() {
    let schema = load_schema();
    let doc = load_doc("inc-001-database-connection-pool-exhaustion.md");
    let refs = collect_outgoing_refs(&doc, &schema);
    assert!(refs.contains(&"ADR-001".to_string()));
    assert!(refs.contains(&"ADR-003".to_string()));
    assert!(refs.contains(&"OPP-001".to_string()));
    assert!(refs.contains(&"POL-001".to_string()));
}

/// Find all docs that reference a given ID.
fn find_docs_referencing(target_id: &str, schema: &Schema) -> Vec<String> {
    let files = discovery::discover_files(fixtures_dir(), None, &[], false).unwrap();
    let mut referencing = Vec::new();

    for path in files {
        let doc = match Document::from_file(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let refs = collect_outgoing_refs(&doc, schema);
        if refs.contains(&target_id.to_string()) {
            let id = md_db::graph::path_to_id(&path);
            referencing.push(id);
        }
    }

    referencing.sort();
    referencing
}

#[test]
fn query_who_references_adr_001() {
    let schema = load_schema();
    let referencing = find_docs_referencing("ADR-001", &schema);
    // ADR-002 (enabled_by, related), OPP-001 (enabled_by), POL-001 (caused_by), INC-001 (caused_by)
    assert!(referencing.contains(&"ADR-002".to_string()));
    assert!(referencing.contains(&"OPP-001".to_string()));
    assert!(referencing.contains(&"POL-001".to_string()));
    assert!(referencing.contains(&"INC-001".to_string()));
}

#[test]
fn query_who_references_opp_001() {
    let schema = load_schema();
    let referencing = find_docs_referencing("OPP-001", &schema);
    // ADR-001 (enables), ADR-002 (related), POL-001 (blocks), INC-001 (enables)
    assert!(referencing.contains(&"ADR-001".to_string()));
    assert!(referencing.contains(&"ADR-002".to_string()));
    assert!(referencing.contains(&"POL-001".to_string()));
    assert!(referencing.contains(&"INC-001".to_string()));
}

#[test]
fn query_who_references_pol_001() {
    let schema = load_schema();
    let referencing = find_docs_referencing("POL-001", &schema);
    // ADR-001 (triggers), OPP-001 (blocked_by), INC-001 (related)
    assert!(referencing.contains(&"ADR-001".to_string()));
    assert!(referencing.contains(&"OPP-001".to_string()));
    assert!(referencing.contains(&"INC-001".to_string()));
}

// ─── Validation report format ────────────────────────────────────────────────

#[test]
fn validation_report_format() {
    let schema = load_schema();
    let result = validation::validate_directory(fixtures_dir(), &schema, None, None).unwrap();
    let report = result.to_report();
    assert!(report.contains("result:"));
    assert!(report.contains("error(s)"));
    assert!(report.contains("warning(s)"));
}

#[test]
fn validation_report_includes_file_paths() {
    let schema = load_schema();
    let result = validation::validate_directory(fixtures_dir(), &schema, None, None).unwrap();
    let report = result.to_report();
    // ADR-003 has a warning
    assert!(report.contains("adr-003-use-redis-for-caching.md"));
}

// ─── Singleton docs ──────────────────────────────────────────────────────────

fn singleton_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/singleton")
}

fn load_singleton_schema() -> Schema {
    Schema::from_file(singleton_fixtures_dir().join("schema.kdl")).unwrap()
}

#[test]
fn singleton_schema_parses() {
    let schema = load_singleton_schema();
    let readme = schema.get_type("readme").unwrap();
    assert!(readme.singleton);
    assert_eq!(readme.match_pattern.as_deref(), Some("README.md"));
    assert!(readme.fields.is_empty());
    assert_eq!(readme.sections.len(), 3);
    assert_eq!(readme.max_count, Some(1));

    let changelog = schema.get_type("changelog").unwrap();
    assert!(changelog.singleton);
    assert_eq!(changelog.match_pattern.as_deref(), Some("CHANGELOG.md"));
}

#[test]
fn singleton_readme_validates() {
    let schema = load_singleton_schema();
    let result =
        validation::validate_directory(singleton_fixtures_dir(), &schema, None, None).unwrap();
    // README.md should be validated as singleton with no errors
    let readme_result = result.file_results.iter().find(|fr| fr.path == "README.md");
    assert!(readme_result.is_some(), "README.md should be validated");
    let fr = readme_result.unwrap();
    assert_eq!(
        fr.errors(),
        0,
        "README.md should have no errors: {:?}",
        fr.diagnostics
    );
}

#[test]
fn singleton_missing_sections_reported() {
    use md_db::document::Document;

    let schema = load_singleton_schema();
    let type_def = schema.get_type("readme").unwrap();

    // A README missing required sections and description after H1
    let doc = Document::from_str("# My Project\n\n## Random\n\nSome text.\n").unwrap();
    let result = validation::validate_singleton(&doc, type_def, None);
    // Should have errors for: missing Install, Usage, License (S010)
    assert!(
        result.errors() >= 3,
        "expected 3+ errors (3 S010), got: {:?}",
        result.diagnostics
    );
    let codes: Vec<&str> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.iter().all(|c| *c == "S010"),
        "all errors should be S010: {:?}",
        codes
    );
}

#[test]
fn singleton_missing_file_detected() {
    let schema = load_singleton_schema();
    let result =
        validation::validate_directory(singleton_fixtures_dir(), &schema, None, None).unwrap();
    // CHANGELOG.md doesn't exist -> should report if it has required sections
    // The changelog type has section "Unreleased" which is NOT required
    // So no error expected for missing CHANGELOG.md
    let changelog_result = result
        .file_results
        .iter()
        .find(|fr| fr.path.contains("CHANGELOG.md"));
    assert!(
        changelog_result.is_none(),
        "CHANGELOG.md without required sections should not trigger error"
    );
}

#[test]
fn singleton_license_file_validates() {
    let schema = load_singleton_schema();
    let result =
        validation::validate_directory(singleton_fixtures_dir(), &schema, None, None).unwrap();
    // LICENSE file exists in fixtures → no L001
    assert!(
        !result
            .file_results
            .iter()
            .any(|fr| fr.diagnostics.iter().any(|d| d.code == "L001")),
        "singleton fixtures with LICENSE file should not trigger L001:\n{}",
        result.to_report()
    );
}

#[test]
fn singleton_license_file_missing() {
    let dir = tempfile::tempdir().unwrap();
    // Copy README.md and schema.kdl but NOT LICENSE
    std::fs::write(
        dir.path().join("README.md"),
        std::fs::read_to_string(singleton_fixtures_dir().join("README.md")).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("schema.kdl"),
        std::fs::read_to_string(singleton_fixtures_dir().join("schema.kdl")).unwrap(),
    )
    .unwrap();

    let schema = load_singleton_schema();
    let result = validation::validate_directory(dir.path(), &schema, None, None).unwrap();
    assert!(
        result
            .file_results
            .iter()
            .any(|fr| fr.diagnostics.iter().any(|d| d.code == "L001")),
        "missing LICENSE file should trigger L001:\n{}",
        result.to_report()
    );
}

#[test]
fn singleton_appears_in_graph() {
    use md_db::graph::DocGraph;

    let schema = load_singleton_schema();
    let graph = DocGraph::build(singleton_fixtures_dir(), &schema).unwrap();
    assert!(
        graph.nodes.contains_key("README"),
        "README should be a graph node, got: {:?}",
        graph.nodes.keys().collect::<Vec<_>>()
    );
    let node = &graph.nodes["README"];
    assert_eq!(node.doc_type.as_deref(), Some("readme"));
}
