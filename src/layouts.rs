use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::analysis::{AnalysisDocument, TreeNodeKind};

pub const MAX_SENTENCE_CHARS: usize = 500;
pub const MAX_NOTES_CHARS: usize = 2_000;
pub const MAX_NICKNAME_CHARS: usize = 40;
pub const MAX_CLASS_CODE_CHARS: usize = 32;
pub const MAX_GRAPH_ITEMS: usize = 500;
const MAX_COORDINATE: f64 = 20_000.0;

pub const SUBMIT_LIMIT: usize = 600;
pub const BAD_CODE_LIMIT: usize = 20;
pub const DEVICE_LIMIT: usize = 30;
pub const RATE_WINDOW: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Bend {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelSide {
    Above,
    Below,
    Right,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelPlacement {
    pub side: LabelSide,
    pub nudges: u32,
    pub anchor: Point,
    pub offset: Point,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeFeedback {
    pub id: String,
    pub label: String,
    pub depth: u32,
    pub has_children: bool,
    pub auto: Point,
    #[serde(rename = "final")]
    pub placed: Point,
    pub label_placement: LabelPlacement,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkFeedback {
    pub source: String,
    pub target: String,
    pub auto: Option<Bend>,
    #[serde(rename = "final")]
    pub placed: Option<Bend>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
    pub compact: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSettings {
    pub row_clearance_px: f64,
    pub group_gap_px: f64,
    pub column_gap_px: f64,
    pub fit_to_frame: bool,
}

/// The frontend's `LayoutFeedback` document (`src/layout-lab-model.ts`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutFeedback {
    pub version: u32,
    pub sentence: String,
    pub saved_at: String,
    pub viewport: Viewport,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<LayoutSettings>,
    pub nodes: Vec<NodeFeedback>,
    pub links: Vec<LinkFeedback>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitLayoutRequest {
    pub class_code: String,
    /// A random id the browser generates once and keeps in localStorage.
    pub device_id: String,
    #[serde(default)]
    pub nickname: Option<String>,
    pub feedback: LayoutFeedback,
}

/// Why a submission was turned away before it reached the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Invalid(String),
    Mismatch(String),
}

fn invalid(message: impl Into<String>) -> Rejection {
    Rejection::Invalid(message.into())
}

/// Uppercase, trimmed, and limited to letters, digits, and dashes.
pub fn normalize_class_code(code: &str) -> Result<String, Rejection> {
    let code = code.trim().to_uppercase();
    let well_formed = !code.is_empty()
        && code.chars().count() <= MAX_CLASS_CODE_CHARS
        && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if well_formed {
        Ok(code)
    } else {
        Err(invalid("The class code is not valid."))
    }
}

pub fn check_device_id(device_id: &str) -> Result<(), Rejection> {
    let well_formed = (16..=64).contains(&device_id.len())
        && device_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if well_formed {
        Ok(())
    } else {
        Err(invalid("The device id is not valid."))
    }
}

/// An empty nickname is no nickname.
pub fn normalize_nickname(nickname: Option<&str>) -> Result<Option<String>, Rejection> {
    let Some(nickname) = nickname.map(str::trim).filter(|name| !name.is_empty()) else {
        return Ok(None);
    };
    if nickname.chars().count() > MAX_NICKNAME_CHARS || nickname.chars().any(char::is_control) {
        return Err(invalid(format!(
            "A nickname is at most {MAX_NICKNAME_CHARS} characters."
        )));
    }
    Ok(Some(nickname.to_owned()))
}

fn in_range(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

fn coordinate(point: Point) -> bool {
    in_range(point.x, -MAX_COORDINATE, MAX_COORDINATE)
        && in_range(point.y, -MAX_COORDINATE, MAX_COORDINATE)
}

fn bend(bend: Option<Bend>) -> bool {
    bend.is_none_or(|b| {
        in_range(b.start, -MAX_COORDINATE, MAX_COORDINATE)
            && in_range(b.end, -MAX_COORDINATE, MAX_COORDINATE)
    })
}

pub fn check_shape(feedback: &LayoutFeedback) -> Result<(), Rejection> {
    if feedback.version != 1 {
        return Err(invalid("Unsupported layout version."));
    }
    let sentence = feedback.sentence.trim();
    if sentence.is_empty() || sentence.chars().count() > MAX_SENTENCE_CHARS {
        return Err(invalid(format!(
            "The sentence must be 1 to {MAX_SENTENCE_CHARS} characters."
        )));
    }
    if feedback.notes.chars().count() > MAX_NOTES_CHARS {
        return Err(invalid(format!(
            "Notes are at most {MAX_NOTES_CHARS} characters."
        )));
    }
    let viewport = &feedback.viewport;
    if !in_range(viewport.width, 1.0, MAX_COORDINATE)
        || !in_range(viewport.height, 1.0, MAX_COORDINATE)
    {
        return Err(invalid("The viewport size is out of range."));
    }
    if let Some(settings) = &feedback.settings {
        let valid = in_range(settings.row_clearance_px, 0.0, 60.0)
            && in_range(settings.group_gap_px, 0.0, 120.0)
            && in_range(settings.column_gap_px, 40.0, 320.0);
        if !valid {
            return Err(invalid("A spacing setting is out of range."));
        }
    }
    if feedback.nodes.len() > MAX_GRAPH_ITEMS || feedback.links.len() > MAX_GRAPH_ITEMS {
        return Err(invalid("The arrangement has too many nodes or links."));
    }
    let nodes_in_frame = feedback.nodes.iter().all(|node| {
        coordinate(node.auto)
            && coordinate(node.placed)
            && coordinate(node.label_placement.anchor)
            && coordinate(node.label_placement.offset)
    });
    let links_in_frame = feedback
        .links
        .iter()
        .all(|link| bend(link.auto) && bend(link.placed));
    if !nodes_in_frame || !links_in_frame {
        return Err(invalid("A position is out of range."));
    }
    Ok(())
}

/// Node ids and parent→child links of the graph the frontend draws for this
/// analysis, following `buildGraphModel` in `src/graph-model.ts`: tree nodes,
/// a `relation-<match id>` child of each sentence for every match on it, and
/// a lone sentence folded into the document.
pub fn expected_graph(
    document: &AnalysisDocument,
) -> (BTreeSet<String>, BTreeSet<(String, String)>) {
    let tree = &document.tree;
    let mut nodes: BTreeSet<String> = tree.nodes.iter().map(|node| node.id.clone()).collect();
    let mut links: BTreeSet<(String, String)> = tree
        .edges
        .iter()
        .map(|edge| (edge.parent_id.clone(), edge.child_id.clone()))
        .collect();

    for node in tree
        .nodes
        .iter()
        .filter(|node| node.kind == TreeNodeKind::Sentence)
    {
        let secondary = node.secondary_match_ids.iter().filter_map(|id| {
            document
                .secondary_matches
                .iter()
                .find(|secondary| &secondary.id == id)
                .map(|secondary| &secondary.matched.id)
        });
        for match_id in node.match_ids.iter().chain(secondary) {
            let relation = format!("relation-{match_id}");
            links.insert((node.id.clone(), relation.clone()));
            nodes.insert(relation);
        }
    }

    let root_children: Vec<&String> = tree
        .edges
        .iter()
        .filter(|edge| edge.parent_id == tree.root_id)
        .map(|edge| &edge.child_id)
        .collect();
    let lone_sentence = match root_children.as_slice() {
        [only] => tree
            .nodes
            .iter()
            .find(|node| &node.id == *only && node.kind == TreeNodeKind::Sentence),
        _ => None,
    };
    if let Some(sentence) = lone_sentence {
        nodes.remove(&sentence.id);
        links = links
            .into_iter()
            .filter(|(_, child)| child != &sentence.id)
            .map(|(parent, child)| {
                if parent == sentence.id {
                    (tree.root_id.clone(), child)
                } else {
                    (parent, child)
                }
            })
            .collect();
    }
    (nodes, links)
}

pub fn check_graph(
    feedback: &LayoutFeedback,
    document: &AnalysisDocument,
) -> Result<(), Rejection> {
    let (nodes, links) = expected_graph(document);
    let sent_nodes: BTreeSet<String> = feedback.nodes.iter().map(|node| node.id.clone()).collect();
    let sent_links: BTreeSet<(String, String)> = feedback
        .links
        .iter()
        .map(|link| (link.source.clone(), link.target.clone()))
        .collect();
    let duplicates =
        sent_nodes.len() != feedback.nodes.len() || sent_links.len() != feedback.links.len();
    if duplicates || sent_nodes != nodes || sent_links != links {
        return Err(Rejection::Mismatch(
            "The arrangement does not match this sentence's graph.".to_owned(),
        ));
    }
    Ok(())
}

/// Numbers that make a careless or hostile arrangement stand out in review.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct Flags {
    pub moved_nodes: i32,
    pub moved_labels: i32,
    pub bent_links: i32,
    /// Total distance the nodes moved, in graph pixels.
    pub displacement_px: f64,
    /// Children left of or level with their parent, plus siblings whose
    /// top-to-bottom order the student swapped.
    pub order_breaks: i32,
}

fn count(items: impl Iterator<Item = bool>) -> i32 {
    i32::try_from(items.filter(|&hit| hit).count()).unwrap_or(i32::MAX)
}

pub fn flags(feedback: &LayoutFeedback) -> Flags {
    let nodes: HashMap<&str, &NodeFeedback> = feedback
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut children: BTreeMap<&str, Vec<&NodeFeedback>> = BTreeMap::new();
    let mut backwards = 0;
    for link in &feedback.links {
        let (Some(parent), Some(child)) = (
            nodes.get(link.source.as_str()),
            nodes.get(link.target.as_str()),
        ) else {
            continue;
        };
        if child.placed.x <= parent.placed.x {
            backwards += 1;
        }
        children
            .entry(link.source.as_str())
            .or_default()
            .push(child);
    }
    let mut swapped = 0;
    for siblings in children.values_mut() {
        siblings.sort_by(|a, b| a.auto.y.total_cmp(&b.auto.y));
        swapped += siblings
            .windows(2)
            .filter(|pair| pair[0].placed.y > pair[1].placed.y)
            .count();
    }
    let displacement: f64 = feedback
        .nodes
        .iter()
        .map(|node| (node.placed.x - node.auto.x).hypot(node.placed.y - node.auto.y))
        .sum();
    Flags {
        moved_nodes: count(feedback.nodes.iter().map(|node| node.auto != node.placed)),
        moved_labels: count(
            feedback
                .nodes
                .iter()
                .map(|node| node.label_placement.offset != Point { x: 0.0, y: 0.0 }),
        ),
        bent_links: count(feedback.links.iter().map(|link| link.auto != link.placed)),
        displacement_px: displacement.round(),
        order_breaks: i32::try_from(backwards + swapped).unwrap_or(i32::MAX),
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    limit: usize,
    seen: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub fn full(&self, client: &str, now: Instant) -> bool {
        let Ok(mut seen) = self.seen.lock() else {
            return false;
        };
        seen.retain(|_, times| {
            times.retain(|&time| now.duration_since(time) < RATE_WINDOW);
            !times.is_empty()
        });
        seen.get(client)
            .is_some_and(|times| times.len() >= self.limit)
    }

    pub fn record(&self, client: &str, now: Instant) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.entry(client.to_owned()).or_default().push_back(now);
        }
    }

    /// Records the attempt unless the client is already at the limit.
    pub fn allow(&self, client: &str, now: Instant) -> bool {
        if self.full(client, now) {
            return false;
        }
        self.record(client, now);
        true
    }
}

/// A salted hash, so the table can tell repeat senders apart without
/// keeping their addresses.
pub fn client_hash(salt: &str, client: &str) -> String {
    Sha256::digest(format!("{salt}\u{0}{client}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone)]
pub struct LayoutStore {
    pool: PgPool,
    salt: Arc<str>,
    submits: Arc<RateLimiter>,
    bad_codes: Arc<RateLimiter>,
    devices: Arc<RateLimiter>,
}

pub struct NewSubmission<'a> {
    pub class_code: &'a str,
    pub nickname: Option<&'a str>,
    pub client: &'a str,
    pub device_id: &'a str,
    pub feedback: &'a LayoutFeedback,
    pub flags: Flags,
}

impl LayoutStore {
    /// Connects and brings the schema up to date.
    pub async fn connect(url: &str, salt: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self::new(pool, salt))
    }

    pub fn new(pool: PgPool, salt: &str) -> Self {
        Self {
            pool,
            salt: Arc::from(salt),
            submits: Arc::new(RateLimiter::new(SUBMIT_LIMIT)),
            bad_codes: Arc::new(RateLimiter::new(BAD_CODE_LIMIT)),
            devices: Arc::new(RateLimiter::new(DEVICE_LIMIT)),
        }
    }

    fn device_hash(&self, device_id: &str) -> String {
        client_hash(&self.salt, &format!("device:{device_id}"))
    }

    /// False once the address sent too many arrangements or wrong codes, or
    /// the device sent too many arrangements.
    pub fn allow(&self, client: &str, device_id: &str) -> bool {
        let client = client_hash(&self.salt, client);
        let device = self.device_hash(device_id);
        let now = Instant::now();
        if self.bad_codes.full(&client, now)
            || self.devices.full(&device, now)
            || self.submits.full(&client, now)
        {
            return false;
        }
        self.devices.record(&device, now);
        self.submits.record(&client, now);
        true
    }

    pub fn record_bad_code(&self, client: &str) {
        self.bad_codes
            .record(&client_hash(&self.salt, client), Instant::now());
    }

    pub async fn class_code_open(&self, code: &str) -> sqlx::Result<bool> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM class_codes WHERE code = $1 AND expires_at > now())",
        )
        .bind(code)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn insert(&self, submission: NewSubmission<'_>) -> anyhow::Result<i64> {
        let feedback = submission.feedback;
        let document = serde_json::to_value(feedback)?;
        let settings = feedback
            .settings
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let flags = submission.flags;
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO layout_submissions \
             (class_code, nickname, client_hash, device_hash, sentence, compact, settings, \
              document, moved_nodes, moved_labels, bent_links, displacement_px, order_breaks) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id",
        )
        .bind(submission.class_code)
        .bind(submission.nickname)
        .bind(client_hash(&self.salt, submission.client))
        .bind(self.device_hash(submission.device_id))
        .bind(feedback.sentence.trim())
        .bind(feedback.viewport.compact)
        .bind(settings)
        .bind(document)
        .bind(flags.moved_nodes)
        .bind(flags.moved_labels)
        .bind(flags.bent_links)
        .bind(flags.displacement_px)
        .bind(flags.order_breaks)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }
}
