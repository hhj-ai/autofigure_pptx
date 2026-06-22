use std::collections::{BTreeMap, HashSet};

use anyhow::{anyhow, Result};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum StyleName {
    #[serde(rename = "wps-clean")]
    WpsClean,
    #[serde(rename = "cvpr-clean")]
    CvprClean,
    #[serde(rename = "neurips-minimal")]
    NeuripsMinimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ImageProviderKind {
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "openai_images")]
    OpenAiImages,
    #[serde(rename = "replicate")]
    Replicate,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ReferencePreviewMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "required")]
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CanvasAspect {
    #[serde(rename = "paper-wide")]
    PaperWide,
    #[serde(rename = "single-column")]
    SingleColumn,
    #[serde(rename = "double-column")]
    DoubleColumn,
    #[serde(rename = "16:9")]
    SixteenNine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Template {
    #[serde(rename = "pipeline")]
    Pipeline,
    #[serde(rename = "teacher_student")]
    TeacherStudent,
    #[serde(rename = "multimodal_fusion")]
    MultimodalFusion,
    #[serde(rename = "training_inference_split")]
    TrainingInferenceSplit,
    #[serde(rename = "module_zoom_in")]
    ModuleZoomIn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FigurePlan {
    #[serde(default = "default_schema_version")]
    pub version: String,
    pub canvas: Canvas,
    pub story: Story,
    pub layout: Layout,
    pub components: Vec<Component>,
    pub edges: Vec<Edge>,
    pub annotations: Vec<Annotation>,
    pub assets: Vec<AssetSpec>,
    pub design: DesignPolicy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DrawPlan {
    #[serde(default = "default_draw_plan_version")]
    pub version: String,
    pub canvas: Canvas,
    #[serde(default)]
    pub style_tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub objects: Vec<DrawObject>,
}

fn default_draw_plan_version() -> String {
    "0.2".to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum DrawObject {
    #[serde(rename = "box")]
    Box {
        id: String,
        bbox: [f64; 4],
        text: String,
        role: String,
        style: String,
        z: i32,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        bbox: [f64; 4],
        text: String,
        style: String,
        z: i32,
    },
    #[serde(rename = "connector")]
    Connector {
        id: String,
        points: Vec<[f64; 2]>,
        from: Option<String>,
        to: Option<String>,
        style: String,
        label: Option<DrawLabel>,
        z: i32,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        bbox: [f64; 4],
        asset_id: String,
        z: i32,
    },
    #[serde(rename = "group")]
    Group {
        id: String,
        bbox: [f64; 4],
        label: Option<String>,
        style: String,
        z: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DrawLabel {
    pub text: String,
    pub bbox: [f64; 4],
}

fn default_schema_version() -> String {
    "0.1".to_string()
}

impl FigurePlan {
    pub fn mock_from_method(
        method: &str,
        style: StyleName,
        aspect: CanvasAspect,
        target_width_mm: u32,
    ) -> Self {
        let lower = method.to_lowercase();
        let template = if lower.contains("teacher") || lower.contains("student") {
            Template::TeacherStudent
        } else if lower.contains("fusion")
            || lower.contains("encoder")
            || lower.contains("multimodal")
        {
            Template::MultimodalFusion
        } else if lower.contains("train") && lower.contains("infer") {
            Template::TrainingInferenceSplit
        } else if lower.contains("zoom") || lower.contains("inside") {
            Template::ModuleZoomIn
        } else {
            Template::Pipeline
        };

        let (components, edges, assets, focus) = match template {
            Template::TeacherStudent => (
                vec![
                    Component::new(
                        "teacher",
                        "Teacher LM",
                        ComponentRole::Context,
                        VisualWeight::Muted,
                        "main_lane",
                    ),
                    Component::new(
                        "student",
                        "Student LM",
                        ComponentRole::Main,
                        VisualWeight::Strong,
                        "main_lane",
                    )
                    .with_asset("student_icon"),
                    Component::new(
                        "output",
                        "Prediction",
                        ComponentRole::Output,
                        VisualWeight::Normal,
                        "main_lane",
                    ),
                ],
                vec![
                    Edge::new(
                        "teacher_to_student",
                        "teacher",
                        "student",
                        "latent residual",
                        EdgeSemantic::Supervision,
                        EdgeStyle::Dash,
                        EdgeImportance::Main,
                    ),
                    Edge::new(
                        "student_to_output",
                        "student",
                        "output",
                        "task output",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                ],
                vec![AssetSpec::generated_icon(
                    "student_icon",
                    "compact student model module",
                )],
                vec!["student".to_string(), "latent residual".to_string()],
            ),
            Template::MultimodalFusion => (
                vec![
                    Component::new(
                        "vision_encoder",
                        "Vision Enc.",
                        ComponentRole::Input,
                        VisualWeight::Normal,
                        "main_lane",
                    )
                    .with_asset("vision_icon"),
                    Component::new(
                        "text_encoder",
                        "Text Enc.",
                        ComponentRole::Input,
                        VisualWeight::Normal,
                        "main_lane",
                    ),
                    Component::new(
                        "fusion",
                        "Fusion",
                        ComponentRole::Main,
                        VisualWeight::Strong,
                        "main_lane",
                    ),
                    Component::new(
                        "head",
                        "Task Head",
                        ComponentRole::Output,
                        VisualWeight::Normal,
                        "main_lane",
                    ),
                ],
                vec![
                    Edge::new(
                        "vision_to_fusion",
                        "vision_encoder",
                        "fusion",
                        "visual tokens",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                    Edge::new(
                        "text_to_fusion",
                        "text_encoder",
                        "fusion",
                        "text tokens",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                    Edge::new(
                        "fusion_to_head",
                        "fusion",
                        "head",
                        "joint state",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                ],
                vec![AssetSpec::generated_icon(
                    "vision_icon",
                    "vision encoder pictogram",
                )],
                vec!["fusion".to_string(), "joint state".to_string()],
            ),
            _ => (
                vec![
                    Component::new(
                        "input",
                        "Input",
                        ComponentRole::Input,
                        VisualWeight::Normal,
                        "main_lane",
                    ),
                    Component::new(
                        "method_core",
                        "Method Core",
                        ComponentRole::Main,
                        VisualWeight::Strong,
                        "main_lane",
                    ),
                    Component::new(
                        "output",
                        "Output",
                        ComponentRole::Output,
                        VisualWeight::Normal,
                        "main_lane",
                    ),
                ],
                vec![
                    Edge::new(
                        "input_to_core",
                        "input",
                        "method_core",
                        "features",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                    Edge::new(
                        "core_to_output",
                        "method_core",
                        "output",
                        "prediction",
                        EdgeSemantic::DataFlow,
                        EdgeStyle::Solid,
                        EdgeImportance::Main,
                    ),
                ],
                vec![AssetSpec::generated_icon(
                    "method_icon",
                    "method module pictogram",
                )],
                vec!["method_core".to_string()],
            ),
        };

        Self {
            version: "0.1".to_string(),
            canvas: Canvas {
                aspect,
                target_width_mm,
                safe_margin: 0.06,
            },
            story: Story {
                main_message: summarize_method(method),
                visual_focus: focus,
                reading_order: ReadingOrder::LeftToRight,
            },
            layout: Layout {
                template,
                grid: Grid {
                    columns: 12,
                    rows: 6,
                },
                regions: vec![LayoutRegion {
                    id: "main_lane".to_string(),
                    bbox: [0.06, 0.22, 0.94, 0.76],
                }],
            },
            components,
            edges,
            annotations: vec![],
            assets,
            design: DesignPolicy {
                style,
                max_colors: 4,
                font_policy: FontPolicy::WpsFriendly,
                avoid_arrow_crossing: true,
                prefer_native_shapes: true,
            },
        }
    }
}

fn summarize_method(method: &str) -> String {
    let compact = method
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    let compact = compact.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 140 {
        compact.chars().take(137).collect::<String>() + "..."
    } else if compact.is_empty() {
        "Method overview".to_string()
    } else {
        compact
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Canvas {
    pub aspect: CanvasAspect,
    pub target_width_mm: u32,
    pub safe_margin: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Story {
    pub main_message: String,
    pub visual_focus: Vec<String>,
    pub reading_order: ReadingOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ReadingOrder {
    #[serde(rename = "left_to_right")]
    LeftToRight,
    #[serde(rename = "top_to_bottom", alias = "bottom_to_top")]
    TopToBottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Layout {
    pub template: Template,
    pub grid: Grid,
    pub regions: Vec<LayoutRegion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Grid {
    pub columns: u32,
    pub rows: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutRegion {
    pub id: String,
    pub bbox: [f64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Component {
    pub id: String,
    pub label: String,
    pub role: ComponentRole,
    pub visual_weight: VisualWeight,
    pub region: String,
    pub allowed_asset_id: Option<String>,
}

impl Component {
    pub fn new(
        id: &str,
        label: &str,
        role: ComponentRole,
        visual_weight: VisualWeight,
        region: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            role,
            visual_weight,
            region: region.to_string(),
            allowed_asset_id: None,
        }
    }

    pub fn with_asset(mut self, asset_id: &str) -> Self {
        self.allowed_asset_id = Some(asset_id.to_string());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ComponentRole {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "context")]
    Context,
    #[serde(rename = "input")]
    Input,
    #[serde(rename = "output")]
    Output,
    #[serde(rename = "loss", alias = "supervision")]
    Loss,
    #[serde(rename = "data")]
    Data,
    #[serde(rename = "module")]
    Module,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum VisualWeight {
    #[serde(rename = "strong")]
    Strong,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "muted")]
    Muted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub semantic: EdgeSemantic,
    pub style: EdgeStyle,
    pub importance: EdgeImportance,
}

impl Edge {
    pub fn new(
        id: &str,
        from: &str,
        to: &str,
        label: &str,
        semantic: EdgeSemantic,
        style: EdgeStyle,
        importance: EdgeImportance,
    ) -> Self {
        Self {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            label: label.to_string(),
            semantic,
            style,
            importance,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EdgeSemantic {
    #[serde(rename = "data_flow")]
    DataFlow,
    #[serde(rename = "supervision")]
    Supervision,
    #[serde(rename = "loss")]
    Loss,
    #[serde(rename = "feedback")]
    Feedback,
    #[serde(rename = "reference")]
    Reference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EdgeStyle {
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "dash")]
    Dash,
    #[serde(rename = "long_dash")]
    LongDash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EdgeImportance {
    #[serde(rename = "main")]
    Main,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "aux")]
    Aux,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub id: String,
    pub label: String,
    pub target_id: Option<String>,
    pub bbox: Option<[f64; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AssetSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: AssetType,
    pub prompt: String,
    pub negative_prompt: String,
    pub usage: AssetUsage,
    pub size: AssetSize,
    pub transparent_background: bool,
    pub style_constraints: AssetStyleConstraints,
    pub status: AssetStatus,
}

impl AssetSpec {
    pub fn generated_icon(id: &str, prompt: &str) -> Self {
        Self {
            id: id.to_string(),
            asset_type: AssetType::GeneratedIcon,
            prompt: prompt.to_string(),
            negative_prompt: "text, letters, numbers, watermark, signature, photorealistic clutter"
                .to_string(),
            usage: AssetUsage::InsideComponent,
            size: AssetSize::Small,
            transparent_background: true,
            style_constraints: AssetStyleConstraints {
                flat: true,
                no_text: true,
                match_palette: StyleName::WpsClean,
            },
            status: AssetStatus::Missing,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssetType {
    #[serde(rename = "generated_icon")]
    GeneratedIcon,
    #[serde(rename = "generated_texture")]
    GeneratedTexture,
    #[serde(rename = "imported")]
    Imported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssetUsage {
    #[serde(rename = "inside_component")]
    InsideComponent,
    #[serde(rename = "background_decoration")]
    BackgroundDecoration,
    #[serde(rename = "thumbnail")]
    Thumbnail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssetSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "wide_strip")]
    WideStrip,
    #[serde(rename = "thumbnail")]
    Thumbnail,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AssetStyleConstraints {
    pub flat: bool,
    pub no_text: bool,
    pub match_palette: StyleName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssetStatus {
    #[serde(rename = "missing")]
    Missing,
    #[serde(rename = "generated")]
    Generated,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "needs_regeneration")]
    NeedsRegeneration,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DesignPolicy {
    pub style: StyleName,
    pub max_colors: u32,
    pub font_policy: FontPolicy,
    pub avoid_arrow_crossing: bool,
    pub prefer_native_shapes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FontPolicy {
    #[serde(rename = "wps_friendly")]
    WpsFriendly,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Review {
    pub passed: bool,
    pub scores: ReviewScores,
    pub blocking_issues: Vec<String>,
    pub localized_issues: Vec<LocalizedIssue>,
    pub accepted_assets: Vec<String>,
    pub rejected_assets: Vec<RejectedAsset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReferenceSelection {
    #[serde(default = "default_reference_selection_version")]
    pub version: String,
    pub selected_reference_id: String,
    pub selected_reference_name: String,
    pub source_paper: String,
    pub source_url: String,
    pub preview_path: Option<String>,
    pub preview_mode: ReferencePreviewMode,
    pub why_fit: String,
    pub adaptation_rules: Vec<String>,
    pub anti_patterns: Vec<String>,
    pub quality_targets: Vec<String>,
}

fn default_reference_selection_version() -> String {
    "0.1".to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RoundImprovementPlan {
    #[serde(default = "default_round_improvement_plan_version")]
    pub version: String,
    pub round_index: u32,
    pub reference_id: String,
    pub summary: String,
    pub actions: Vec<ImprovementAction>,
    pub preserve: Vec<String>,
    pub rejected_as_unusable: bool,
}

fn default_round_improvement_plan_version() -> String {
    "0.1".to_string()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImprovementAction {
    pub target_id: Option<String>,
    pub change_type: String,
    pub issue: String,
    pub expected_visible_effect: String,
    pub success_check: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewScores {
    pub semantic_fidelity: u8,
    pub story_clarity: u8,
    pub visual_hierarchy: u8,
    pub paper_readability: u8,
    pub layout_cleanliness: u8,
    pub arrow_routing: u8,
    pub color_semantics: u8,
    pub aesthetic_quality: u8,
    pub wps_editability: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LocalizedIssue {
    pub target_id: String,
    pub bbox: [f64; 4],
    pub severity: IssueSeverity,
    pub issue: String,
    pub evidence: String,
    pub suggested_direction: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IssueSeverity {
    #[serde(rename = "blocking")]
    Blocking,
    #[serde(rename = "major")]
    Major,
    #[serde(rename = "minor")]
    Minor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RejectedAsset {
    pub asset_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatchPlan {
    pub operations: Vec<PatchOperation>,
    pub stop_reason: PatchStopReason,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatchOperation {
    pub id: String,
    pub target_id: String,
    pub executor: PatchExecutor,
    pub operation_type: PatchOperationType,
    pub action: String,
    pub expected_effect: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchExecutor {
    #[serde(rename = "reasoner")]
    Reasoner,
    #[serde(rename = "coder")]
    Coder,
    #[serde(rename = "image_model")]
    ImageModel,
    #[serde(rename = "agent")]
    Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchOperationType {
    #[serde(rename = "layout_patch")]
    LayoutPatch,
    #[serde(rename = "style_patch")]
    StylePatch,
    #[serde(rename = "text_patch")]
    TextPatch,
    #[serde(rename = "asset_regeneration")]
    AssetRegeneration,
    #[serde(rename = "edge_reroute")]
    EdgeReroute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PatchStopReason {
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "cap_reached")]
    CapReached,
}

pub fn validate_stable_ids(plan: &FigurePlan) -> Result<()> {
    let mut ids = HashSet::new();
    for (kind, id) in collect_ids(plan) {
        if id.trim().is_empty() {
            return Err(anyhow!("{kind} has empty stable id"));
        }
        if !ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate stable id: {id}"));
        }
    }

    let component_ids: HashSet<_> = plan
        .components
        .iter()
        .map(|component| component.id.as_str())
        .collect();
    for edge in &plan.edges {
        if !component_ids.contains(edge.from.as_str()) {
            return Err(anyhow!(
                "edge {} references missing from component {}",
                edge.id,
                edge.from
            ));
        }
        if !component_ids.contains(edge.to.as_str()) {
            return Err(anyhow!(
                "edge {} references missing to component {}",
                edge.id,
                edge.to
            ));
        }
    }

    Ok(())
}

pub fn validate_draw_plan(plan: &DrawPlan) -> Result<()> {
    if plan.version.trim().is_empty() {
        return Err(anyhow!("DrawPlan has empty version"));
    }

    let mut ids = HashSet::new();
    for object in &plan.objects {
        let id = draw_object_id(object);
        if id.trim().is_empty() {
            return Err(anyhow!("draw object has empty id"));
        }
        if !ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate draw object id: {id}"));
        }

        match object {
            DrawObject::Box { bbox, .. }
            | DrawObject::Text { bbox, .. }
            | DrawObject::Group { bbox, .. } => validate_draw_bbox(id, *bbox)?,
            DrawObject::Image { bbox, asset_id, .. } => {
                validate_draw_bbox(id, *bbox)?;
                if asset_id.trim().is_empty() {
                    return Err(anyhow!("draw image {id} has empty asset_id"));
                }
                if draw_bbox_area(*bbox) > 0.45 {
                    return Err(anyhow!(
                        "full-slide raster image is not allowed in DrawPlan: {id}"
                    ));
                }
            }
            DrawObject::Connector { points, label, .. } => {
                if points.len() < 2 {
                    return Err(anyhow!("draw connector {id} needs at least two points"));
                }
                for point in points {
                    validate_draw_point(id, *point)?;
                }
                if let Some(label) = label {
                    validate_draw_bbox(id, label.bbox)?;
                }
            }
        }
    }
    Ok(())
}

fn draw_object_id(object: &DrawObject) -> &str {
    match object {
        DrawObject::Box { id, .. }
        | DrawObject::Text { id, .. }
        | DrawObject::Connector { id, .. }
        | DrawObject::Image { id, .. }
        | DrawObject::Group { id, .. } => id,
    }
}

fn validate_draw_bbox(id: &str, bbox: [f64; 4]) -> Result<()> {
    if bbox.iter().any(|value| !value.is_finite()) {
        return Err(anyhow!("draw object {id} has non-finite bbox"));
    }
    if bbox.iter().any(|value| *value < 0.0 || *value > 1.0) {
        return Err(anyhow!(
            "draw object {id} bbox is outside normalized canvas"
        ));
    }
    if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
        return Err(anyhow!("draw object {id} bbox has non-positive size"));
    }
    Ok(())
}

fn validate_draw_point(id: &str, point: [f64; 2]) -> Result<()> {
    if point.iter().any(|value| !value.is_finite()) {
        return Err(anyhow!("draw connector {id} has non-finite point"));
    }
    if point.iter().any(|value| *value < 0.0 || *value > 1.0) {
        return Err(anyhow!(
            "draw connector {id} point is outside normalized canvas"
        ));
    }
    Ok(())
}

fn draw_bbox_area(bbox: [f64; 4]) -> f64 {
    ((bbox[2] - bbox[0]).max(0.0)) * ((bbox[3] - bbox[1]).max(0.0))
}

fn collect_ids(plan: &FigurePlan) -> Vec<(&'static str, &str)> {
    let mut ids = Vec::new();
    for region in &plan.layout.regions {
        ids.push(("layout region", region.id.as_str()));
    }
    for component in &plan.components {
        ids.push(("component", component.id.as_str()));
    }
    for edge in &plan.edges {
        ids.push(("edge", edge.id.as_str()));
    }
    for annotation in &plan.annotations {
        ids.push(("annotation", annotation.id.as_str()));
    }
    for asset in &plan.assets {
        ids.push(("asset", asset.id.as_str()));
    }
    ids
}

pub fn figure_plan_schema_json() -> Result<String> {
    let schema = schema_for!(FigurePlan);
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn draw_plan_schema_json() -> Result<String> {
    let schema = schema_for!(DrawPlan);
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn review_schema_json() -> Result<String> {
    let schema = schema_for!(Review);
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn patch_plan_schema_json() -> Result<String> {
    let schema = schema_for!(PatchPlan);
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn reference_selection_schema_json() -> Result<String> {
    let schema = schema_for!(ReferenceSelection);
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn round_improvement_plan_schema_json() -> Result<String> {
    let schema = schema_for!(RoundImprovementPlan);
    Ok(serde_json::to_string_pretty(&schema)?)
}
