export type TreeNodeKind = "document" | "sentence" | "bunsetsu" | "token";
export type SecondaryReason =
  | "contained_by_stronger_match"
  | "overlaps_stronger_match";

export interface CatalogSource {
  id: string;
  label: string;
}

export interface PatternCapture {
  name: string;
  token_start: number;
  token_end: number;
}

export interface MatchScore {
  fallback: boolean;
  priority: number;
  span_length: number;
  core_specificity: number;
  context_specificity: number;
  wildcard_steps: number;
  optional_steps: number;
}

export interface MatchProvenance {
  source: CatalogSource;
  rule_id: string;
  variant_id: string;
}

export interface DisplayMatch {
  id: string;
  rule_name: string;
  jlpt: string;
  meaning_en: string;
  hint: string | null;
  sense_id: string | null;
  ambiguity_group: string | null;
  captures: PatternCapture[];
  token_start: number;
  token_end: number;
  score: MatchScore;
  provenance: MatchProvenance[];
}

export interface SecondaryMatch {
  id: string;
  matched: DisplayMatch;
  reason: SecondaryReason;
  blocked_by: string | null;
}

export interface DictionaryGloss {
  entry_seq: number;
  gloss: string;
  pos: string[];
}

export interface AnalyzedToken {
  id: string;
  surface: string;
  pos1: string;
  pos2: string;
  pos3: string;
  pos4: string;
  conj_type: string;
  conj_form: string;
  base_form: string;
  reading: string;
  byte_start: number;
  byte_end: number;
  position: number;
  glosses: DictionaryGloss[];
}

export interface AnalysisTreeNode {
  id: string;
  kind: TreeNodeKind;
  token_start: number | null;
  token_end: number | null;
  token_id: string | null;
  /** Primary matches this node is the smallest cover of. */
  match_ids: string[];
  secondary_match_ids: string[];
}

export interface AnalysisTreeEdge {
  parent_id: string;
  child_id: string;
  order: number;
}

export interface AnalysisTree {
  root_id: string;
  nodes: AnalysisTreeNode[];
  edges: AnalysisTreeEdge[];
}

export interface AnalysisDocument {
  schema_version: 2;
  input: string;
  tokens: AnalyzedToken[];
  primary_matches: DisplayMatch[];
  secondary_matches: SecondaryMatch[];
  tree: AnalysisTree;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseAnalysisDocument(value: unknown): AnalysisDocument {
  if (!isRecord(value)) {
    throw new Error("invalid analysis document");
  }
  if (value.schema_version !== 2) {
    throw new Error(`unsupported analysis schema version: ${String(value.schema_version)}`);
  }
  if (
    typeof value.input !== "string" ||
    !Array.isArray(value.tokens) ||
    !Array.isArray(value.primary_matches) ||
    !Array.isArray(value.secondary_matches) ||
    !isRecord(value.tree) ||
    typeof value.tree.root_id !== "string" ||
    !Array.isArray(value.tree.nodes) ||
    !Array.isArray(value.tree.edges)
  ) {
    throw new Error("invalid analysis document");
  }
  return value as unknown as AnalysisDocument;
}
