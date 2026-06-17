import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { type LayoutMap, type LayoutObject, type NormalizedBox, normalizeBox } from "./safe_api.js";

const require = createRequire(import.meta.url);
const PptxGenJS = require("pptxgenjs") as new () => any;

interface RenderPayload {
  out_dir: string;
  plan: FigurePlan;
  style: StyleSpec;
  asset_paths: Record<string, string>;
}

interface FigurePlan {
  canvas: {
    aspect: "paper-wide" | "single-column" | "double-column" | "16:9";
    target_width_mm: number;
    safe_margin: number;
  };
  story: {
    main_message: string;
    visual_focus: string[];
    reading_order: string;
  };
  layout: {
    template: string;
    regions: Array<{ id: string; bbox: NormalizedBox }>;
  };
  components: Component[];
  edges: Edge[];
  annotations: Array<{ id: string; label: string; target_id?: string; bbox?: NormalizedBox }>;
}

interface Component {
  id: string;
  label: string;
  role: string;
  visual_weight: "strong" | "normal" | "muted";
  region: string;
  allowed_asset_id?: string | null;
}

interface Edge {
  id: string;
  from: string;
  to: string;
  label: string;
  semantic: string;
  style: "solid" | "dash" | "long_dash";
  importance: "main" | "normal" | "aux";
}

interface StyleSpec {
  fonts: {
    font_cjk: string;
    font_latin: string;
    font_mono: string;
  };
  palette: {
    background: string;
    text: string;
    muted_text: string;
    stroke: string;
    muted_fill: string;
    primary: string;
    accent: string;
    warning: string;
  };
  line_widths: {
    auxiliary: number;
    normal: number;
    main_flow: number;
    strong_focus: number;
  };
  corner_radius: {
    module: number;
    group: number;
  };
  font_sizes: {
    module_label: number;
    auxiliary_label: number;
    section_label: number;
    min_label: number;
  };
}

interface CanvasSize {
  width: number;
  height: number;
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function createFigureRuntime(payload: RenderPayload): FigureRuntime {
  return new FigureRuntime(payload);
}

export class FigureRuntime {
  private pptx: any;
  private slide: any;
  private objects: LayoutObject[] = [];
  private size: CanvasSize;

  constructor(private readonly payload: RenderPayload) {
    this.size = canvasSize(payload.plan.canvas.aspect);
  }

  async renderPlan(): Promise<void> {
    fs.mkdirSync(this.payload.out_dir, { recursive: true });
    this.pptx = new PptxGenJS();
    const layoutName = `METHODFIG_${Date.now()}`;
    this.pptx.defineLayout({ name: layoutName, width: this.size.width, height: this.size.height });
    this.pptx.layout = layoutName;
    this.pptx.author = "methodfig";
    this.pptx.company = "methodfig";
    this.pptx.subject = "Paper method overview figure";
    this.pptx.title = "methodfig generated editable figure";
    this.pptx.lang = "zh-CN";

    this.slide = this.pptx.addSlide();
    this.slide.background = { color: this.payload.style.palette.background };

    this.drawRegions();
    const componentBoxes = this.layoutComponents();
    this.drawEdges(componentBoxes);
    this.drawComponents(componentBoxes);
    this.drawAnnotations();

    const layoutMap: LayoutMap = {
      canvas: {
        width: this.size.width,
        height: this.size.height,
        aspect: this.payload.plan.canvas.aspect,
        target_width_mm: this.payload.plan.canvas.target_width_mm
      },
      objects: this.objects
    };

    fs.writeFileSync(
      path.join(this.payload.out_dir, "layout_map.json"),
      JSON.stringify(layoutMap, null, 2),
      "utf8"
    );
    await this.pptx.writeFile({ fileName: path.join(this.payload.out_dir, "figure.pptx") });
  }

  private drawRegions(): void {
    for (const region of this.payload.plan.layout.regions) {
      const box = this.toBox(region.bbox);
      this.slide.addShape(this.shape("rect"), {
        ...box,
        fill: { color: this.payload.style.palette.background, transparency: 100 },
        line: { color: this.payload.style.palette.stroke, transparency: 100, width: 0.75 },
        shapeName: `methodfig_region_${region.id}`
      });
      this.track(region.id, "region", region.bbox);
    }
  }

  private drawComponents(boxes: Map<string, NormalizedBox>): void {
    for (const component of this.payload.plan.components) {
      const normalized = boxes.get(component.id);
      if (!normalized) continue;
      const box = this.toBox(normalized);
      const isStrong = component.visual_weight === "strong";
      const isMuted = component.visual_weight === "muted";
      const fill = isStrong ? lighten(this.payload.style.palette.primary, 0.86) : this.payload.style.palette.muted_fill;
      const stroke = isStrong ? this.payload.style.palette.primary : this.payload.style.palette.stroke;
      const text = isMuted ? this.payload.style.palette.muted_text : this.payload.style.palette.text;
      const width = isStrong ? this.payload.style.line_widths.strong_focus : this.payload.style.line_widths.normal;

      this.slide.addText(component.label, {
        ...box,
        shape: this.shape("roundRect"),
        rectRadius: this.payload.style.corner_radius.module,
        margin: 0.05,
        fit: "shrink",
        breakLine: false,
        valign: "mid",
        align: "center",
        fontFace: this.payload.style.fonts.font_cjk,
        fontSize: this.payload.style.font_sizes.module_label,
        bold: isStrong,
        color: text,
        fill: { color: fill },
        line: { color: stroke, width },
        shapeName: `methodfig_component_${component.id}`
      });

      if (component.allowed_asset_id) {
        this.drawAsset(component.allowed_asset_id, normalized);
      }

      this.track(component.id, "component", normalized);
    }
  }

  private drawAsset(assetId: string, componentBox: NormalizedBox): void {
    const assetPath = this.payload.asset_paths[assetId];
    if (!assetPath || !fs.existsSync(assetPath)) return;
    const [x1, y1, x2, y2] = componentBox;
    const iconSize = Math.min(x2 - x1, y2 - y1) * 0.33;
    const box = this.toBox([x1 + 0.02, y1 + 0.05, x1 + 0.02 + iconSize, y1 + 0.05 + iconSize]);
    this.slide.addImage({
      path: assetPath,
      ...box,
      transparency: 0
    });
  }

  private drawEdges(boxes: Map<string, NormalizedBox>): void {
    for (const edge of this.payload.plan.edges) {
      const from = boxes.get(edge.from);
      const to = boxes.get(edge.to);
      if (!from || !to) continue;
      const start = anchor(from, to);
      const end = anchor(to, from);
      const lineBox = this.toLine(start, end);
      const isMain = edge.importance === "main";
      const isSupervision = edge.semantic === "supervision" || edge.semantic === "loss" || edge.style !== "solid";
      const color = isSupervision ? this.payload.style.palette.accent : this.payload.style.palette.primary;
      const width = isMain ? this.payload.style.line_widths.main_flow : this.payload.style.line_widths.normal;

      this.slide.addShape(this.shape("line"), {
        ...lineBox,
        line: {
          color,
          width,
          dash: dashType(edge.style),
          dashType: dashType(edge.style),
          endArrowType: "triangle"
        },
        shapeName: `methodfig_edge_${edge.id}`
      });

      if (edge.label) {
        const labelBox = edgeLabelBox(start, end);
        this.slide.addText(edge.label, {
          ...this.toBox(labelBox),
          margin: 0.01,
          fit: "shrink",
          align: "center",
          valign: "mid",
          fontFace: this.payload.style.fonts.font_cjk,
          fontSize: this.payload.style.font_sizes.auxiliary_label,
          color: this.payload.style.palette.muted_text,
          fill: { color: this.payload.style.palette.background, transparency: 8 },
          line: { color: this.payload.style.palette.background, transparency: 100 },
          shapeName: `methodfig_edge_label_${edge.id}`
        });
      }

      this.track(edge.id, "edge", normalizeBox([start[0], start[1], end[0], end[1]]));
    }
  }

  private drawAnnotations(): void {
    for (const annotation of this.payload.plan.annotations) {
      if (!annotation.bbox) continue;
      const box = this.toBox(annotation.bbox);
      this.slide.addText(annotation.label, {
        ...box,
        margin: 0.03,
        fit: "shrink",
        align: "center",
        valign: "mid",
        fontFace: this.payload.style.fonts.font_cjk,
        fontSize: this.payload.style.font_sizes.auxiliary_label,
        color: this.payload.style.palette.muted_text,
        fill: { color: this.payload.style.palette.background, transparency: 100 },
        line: { color: this.payload.style.palette.stroke, width: this.payload.style.line_widths.auxiliary, dashType: "dash" },
        shapeName: `methodfig_annotation_${annotation.id}`
      });
      this.track(annotation.id, "annotation", annotation.bbox);
    }
  }

  private layoutComponents(): Map<string, NormalizedBox> {
    const template = this.payload.plan.layout.template;
    if (template === "teacher_student") return this.teacherStudentLayout();
    if (template === "multimodal_fusion") return this.multimodalFusionLayout();
    if (template === "training_inference_split") return this.trainingInferenceLayout();
    if (template === "module_zoom_in") return this.moduleZoomLayout();
    return this.pipelineLayout();
  }

  private pipelineLayout(): Map<string, NormalizedBox> {
    const boxes = new Map<string, NormalizedBox>();
    const components = this.payload.plan.components;
    const n = Math.max(components.length, 1);
    const margin = this.payload.plan.canvas.safe_margin;
    const gap = 0.035;
    const usable = 1 - margin * 2 - gap * (n - 1);
    const width = Math.min(0.22, usable / n);
    const startX = (1 - (width * n + gap * (n - 1))) / 2;
    components.forEach((component, index) => {
      const x1 = startX + index * (width + gap);
      boxes.set(component.id, [x1, 0.38, x1 + width, 0.62]);
    });
    return boxes;
  }

  private teacherStudentLayout(): Map<string, NormalizedBox> {
    const boxes = new Map<string, NormalizedBox>();
    const ids = new Set(this.payload.plan.components.map(component => component.id));
    if (ids.has("teacher")) boxes.set("teacher", [0.11, 0.18, 0.34, 0.38]);
    if (ids.has("student")) boxes.set("student", [0.38, 0.36, 0.66, 0.64]);
    if (ids.has("output")) boxes.set("output", [0.73, 0.39, 0.91, 0.61]);
    return fillMissingBoxes(boxes, this.pipelineLayout());
  }

  private multimodalFusionLayout(): Map<string, NormalizedBox> {
    const boxes = new Map<string, NormalizedBox>();
    const ids = new Set(this.payload.plan.components.map(component => component.id));
    if (ids.has("vision_encoder")) boxes.set("vision_encoder", [0.08, 0.22, 0.29, 0.42]);
    if (ids.has("text_encoder")) boxes.set("text_encoder", [0.08, 0.58, 0.29, 0.78]);
    if (ids.has("fusion")) boxes.set("fusion", [0.43, 0.37, 0.64, 0.63]);
    if (ids.has("head")) boxes.set("head", [0.75, 0.41, 0.92, 0.59]);
    return fillMissingBoxes(boxes, this.pipelineLayout());
  }

  private trainingInferenceLayout(): Map<string, NormalizedBox> {
    const boxes = new Map<string, NormalizedBox>();
    const components = this.payload.plan.components;
    components.forEach((component, index) => {
      const row = component.role === "loss" ? 0.66 : 0.34;
      const x1 = 0.08 + index * 0.24;
      boxes.set(component.id, [x1, row, x1 + 0.18, row + 0.18]);
    });
    return boxes;
  }

  private moduleZoomLayout(): Map<string, NormalizedBox> {
    const boxes = this.pipelineLayout();
    const main = this.payload.plan.components.find(component => component.role === "main");
    if (main) {
      boxes.set(main.id, [0.36, 0.34, 0.61, 0.64]);
      boxes.set(`${main.id}_inset`, [0.63, 0.16, 0.92, 0.38]);
    }
    return boxes;
  }

  private toBox(normalized: NormalizedBox): Box {
    const [x1, y1, x2, y2] = normalizeBox(normalized);
    return {
      x: x1 * this.size.width,
      y: y1 * this.size.height,
      w: (x2 - x1) * this.size.width,
      h: (y2 - y1) * this.size.height
    };
  }

  private toLine(start: [number, number], end: [number, number]): Box {
    return {
      x: start[0] * this.size.width,
      y: start[1] * this.size.height,
      w: (end[0] - start[0]) * this.size.width,
      h: (end[1] - start[1]) * this.size.height
    };
  }

  private shape(name: "rect" | "roundRect" | "line"): string {
    const shapeType = this.pptx.ShapeType as Record<string, string>;
    if (name === "roundRect") return shapeType.roundRect ?? shapeType.rect;
    return shapeType[name];
  }

  private track(id: string, kind: LayoutObject["kind"], bbox: NormalizedBox): void {
    this.objects.push({ id, kind, bbox: normalizeBox(bbox) });
  }
}

function canvasSize(aspect: FigurePlan["canvas"]["aspect"]): CanvasSize {
  if (aspect === "single-column") return { width: 3.35, height: 2.55 };
  if (aspect === "16:9") return { width: 10, height: 5.625 };
  return { width: 7.1, height: 3.2 };
}

function anchor(from: NormalizedBox, to: NormalizedBox): [number, number] {
  const [fx1, fy1, fx2, fy2] = from;
  const [tx1, ty1, tx2, ty2] = to;
  const fromCenter: [number, number] = [(fx1 + fx2) / 2, (fy1 + fy2) / 2];
  const toCenter: [number, number] = [(tx1 + tx2) / 2, (ty1 + ty2) / 2];
  const dx = toCenter[0] - fromCenter[0];
  const dy = toCenter[1] - fromCenter[1];
  if (Math.abs(dx) >= Math.abs(dy)) {
    return [dx >= 0 ? fx2 : fx1, fromCenter[1]];
  }
  return [fromCenter[0], dy >= 0 ? fy2 : fy1];
}

function edgeLabelBox(start: [number, number], end: [number, number]): NormalizedBox {
  const mx = (start[0] + end[0]) / 2;
  const my = (start[1] + end[1]) / 2;
  return normalizeBox([mx - 0.07, my - 0.035, mx + 0.07, my + 0.035]);
}

function dashType(style: Edge["style"]): string {
  if (style === "dash") return "dash";
  if (style === "long_dash") return "lgDash";
  return "solid";
}

function fillMissingBoxes(boxes: Map<string, NormalizedBox>, fallback: Map<string, NormalizedBox>): Map<string, NormalizedBox> {
  for (const [id, box] of fallback.entries()) {
    if (!boxes.has(id)) boxes.set(id, box);
  }
  return boxes;
}

function lighten(hex: string, amount: number): string {
  const clean = hex.replace("#", "");
  const parts = [clean.slice(0, 2), clean.slice(2, 4), clean.slice(4, 6)].map(part => parseInt(part, 16));
  return parts
    .map(value => Math.round(value + (255 - value) * amount).toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();
}
