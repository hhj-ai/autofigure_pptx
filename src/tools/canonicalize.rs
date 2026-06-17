use crate::schema::{
    Component, ComponentRole, Edge, EdgeImportance, EdgeSemantic, EdgeStyle, FigurePlan,
    ImageProviderKind, LayoutRegion, Template, VisualWeight,
};

pub fn canonicalize_plan_for_render(plan: &mut FigurePlan, image_provider: ImageProviderKind) {
    if matches!(image_provider, ImageProviderKind::None) {
        strip_image_asset_expectations(plan);
    }

    if matches!(plan.layout.template, Template::TeacherStudent) {
        canonicalize_teacher_student(plan);
    }
}

fn strip_image_asset_expectations(plan: &mut FigurePlan) {
    plan.assets.clear();
    for component in &mut plan.components {
        component.allowed_asset_id = None;
    }
}

fn canonicalize_teacher_student(plan: &mut FigurePlan) {
    let mut ids = TeacherStudentIds {
        input: find_component(plan, |component| {
            component.role == ComponentRole::Input
                || contains_any(component, &["input", "task", "sample"])
        }),
        teacher: find_component(plan, |component| {
            semantic_module_candidate(component)
                && contains_any(component, &["teacher", "large", "frozen"])
                && !matches!(
                    component.role,
                    ComponentRole::Input | ComponentRole::Output | ComponentRole::Loss
                )
        }),
        student: find_component(plan, |component| {
            semantic_module_candidate(component)
                && contains_any(component, &["student", "compact"])
                && !contains_any(component, &["inference", "deployed", "only"])
                && !contains_any(component, &["output", "answer", "prediction", "predicted"])
                && !matches!(
                    component.role,
                    ComponentRole::Input | ComponentRole::Output | ComponentRole::Loss
                )
        }),
        residual: find_component(plan, |component| {
            semantic_module_candidate(component)
                && contains_any(
                    component,
                    &["residual", "latent", "h_t", "h_s", "z_teacher"],
                )
                && !contains_any(component, &["teacher model", "student model"])
        }),
        task_loss: find_component(plan, |component| {
            semantic_module_candidate(component)
                && contains_any(component, &["task loss", "l_task", "loss"])
                && !contains_any(component, &["residual", "latent"])
        }),
        output: find_component(plan, |component| {
            semantic_module_candidate(component)
                && !contains_any(component, &["latent", "residual", "teacher", "inference"])
                && (component.role == ComponentRole::Output
                    || contains_any(component, &["output", "answer", "prediction", "predicted"]))
        }),
        inference: find_component(plan, |component| {
            contains_any(component, &["inference", "deployed", "student only"])
        }),
        inference_student: None,
    };

    ensure_teacher_student_output(plan, &mut ids);
    ensure_teacher_student_inference_student(plan, &mut ids);
    plan.layout.regions = teacher_student_regions();
    assign_component_regions(plan, &ids);
    prune_noncanonical_teacher_student_components(plan, &ids);
    prune_unused_teacher_student_regions(plan);
    canonicalize_teacher_student_edges(plan, &ids);
    canonicalize_teacher_student_annotations(plan);
    if !plan
        .story
        .visual_focus
        .iter()
        .any(|focus| focus.eq_ignore_ascii_case("teacher-student residual supervision"))
    {
        plan.story
            .visual_focus
            .push("teacher-student residual supervision".to_string());
    }
}

#[derive(Default)]
struct TeacherStudentIds {
    input: Option<String>,
    teacher: Option<String>,
    student: Option<String>,
    residual: Option<String>,
    task_loss: Option<String>,
    output: Option<String>,
    inference: Option<String>,
    inference_student: Option<String>,
}

fn teacher_student_regions() -> Vec<LayoutRegion> {
    vec![
        LayoutRegion {
            id: "ts_input".to_string(),
            bbox: [0.06, 0.44, 0.17, 0.60],
        },
        LayoutRegion {
            id: "ts_teacher".to_string(),
            bbox: [0.24, 0.10, 0.42, 0.28],
        },
        LayoutRegion {
            id: "ts_student".to_string(),
            bbox: [0.24, 0.48, 0.42, 0.68],
        },
        LayoutRegion {
            id: "ts_residual".to_string(),
            bbox: [0.43, 0.24, 0.60, 0.44],
        },
        LayoutRegion {
            id: "ts_task_loss".to_string(),
            bbox: [0.44, 0.58, 0.60, 0.74],
        },
        LayoutRegion {
            id: "ts_output".to_string(),
            bbox: [0.76, 0.58, 0.94, 0.76],
        },
        LayoutRegion {
            id: "ts_inference_student".to_string(),
            bbox: [0.62, 0.28, 0.82, 0.48],
        },
        LayoutRegion {
            id: "ts_inference_note".to_string(),
            bbox: [0.62, 0.12, 0.82, 0.22],
        },
        LayoutRegion {
            id: "ts_aux".to_string(),
            bbox: [0.62, 0.78, 0.82, 0.90],
        },
    ]
}

fn assign_component_regions(plan: &mut FigurePlan, ids: &TeacherStudentIds) {
    for component in &mut plan.components {
        if Some(component.id.as_str()) == ids.input.as_deref() {
            component.region = "ts_input".to_string();
            component.visual_weight = VisualWeight::Normal;
        } else if Some(component.id.as_str()) == ids.teacher.as_deref() {
            component.region = "ts_teacher".to_string();
            component.visual_weight = VisualWeight::Normal;
            shorten_label_if_needed(component, "Teacher\n(large)");
        } else if Some(component.id.as_str()) == ids.student.as_deref() {
            component.region = "ts_student".to_string();
            component.visual_weight = VisualWeight::Strong;
            shorten_label_if_needed(component, "Student\n(training)");
        } else if Some(component.id.as_str()) == ids.residual.as_deref() {
            component.region = "ts_residual".to_string();
            component.visual_weight = VisualWeight::Strong;
            shorten_label_if_needed(component, "Residual\nh_T - h_S");
        } else if Some(component.id.as_str()) == ids.task_loss.as_deref() {
            component.region = "ts_task_loss".to_string();
            component.visual_weight = VisualWeight::Normal;
            shorten_label_if_needed(component, "Task Loss");
        } else if Some(component.id.as_str()) == ids.output.as_deref() {
            component.region = "ts_output".to_string();
            component.visual_weight = VisualWeight::Normal;
            shorten_label_if_needed(component, "Prediction");
        } else if Some(component.id.as_str()) == ids.inference_student.as_deref() {
            component.region = "ts_inference_student".to_string();
            component.visual_weight = VisualWeight::Normal;
            component.label = "Student\n(inference)".to_string();
        } else if Some(component.id.as_str()) == ids.inference.as_deref() {
            component.region = "ts_inference_note".to_string();
            component.visual_weight = VisualWeight::Muted;
            component.label = "Inference:\nstudent only".to_string();
        } else if contains_any(
            component,
            &[
                "training",
                "inference",
                "phase",
                "only",
                "label",
                "title",
                "overview",
                "header",
            ],
        ) {
            component.region = "ts_aux".to_string();
            component.visual_weight = VisualWeight::Muted;
        } else if contains_any(
            component,
            &["teacher", "student", "loss", "residual", "latent"],
        ) {
            component.region = "ts_aux".to_string();
            component.visual_weight = VisualWeight::Muted;
        }
    }
}

fn ensure_teacher_student_output(plan: &mut FigurePlan, ids: &mut TeacherStudentIds) {
    if ids.output.is_some() || ids.student.is_none() {
        return;
    }

    let output_id = unique_id(plan, "ts_prediction", |plan, id| {
        plan.components.iter().any(|component| component.id == id)
    });
    plan.components.push(Component::new(
        &output_id,
        "Prediction",
        ComponentRole::Output,
        VisualWeight::Normal,
        "ts_output",
    ));
    ids.output = Some(output_id);
}

fn ensure_teacher_student_inference_student(plan: &mut FigurePlan, ids: &mut TeacherStudentIds) {
    if ids.inference_student.is_some() || ids.student.is_none() {
        return;
    }

    let inference_student_id = unique_id(plan, "c_inference_student", |plan, id| {
        plan.components.iter().any(|component| component.id == id)
    });
    plan.components.push(Component::new(
        &inference_student_id,
        "Student\n(inference)",
        ComponentRole::Module,
        VisualWeight::Normal,
        "ts_inference_student",
    ));
    ids.inference_student = Some(inference_student_id);
}

fn prune_noncanonical_teacher_student_components(plan: &mut FigurePlan, ids: &TeacherStudentIds) {
    plan.components
        .retain(|component| teacher_student_kept_component(component.id.as_str(), ids));
}

fn teacher_student_kept_component(component_id: &str, ids: &TeacherStudentIds) -> bool {
    [
        ids.input.as_deref(),
        ids.teacher.as_deref(),
        ids.student.as_deref(),
        ids.residual.as_deref(),
        ids.task_loss.as_deref(),
        ids.output.as_deref(),
        ids.inference_student.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|id| id == component_id)
}

fn canonicalize_teacher_student_edges(plan: &mut FigurePlan, ids: &TeacherStudentIds) {
    let Some(student) = ids.student.as_deref() else {
        return;
    };
    let input = ids.input.as_deref();
    let teacher = ids.teacher.as_deref();
    let residual = ids.residual.as_deref();
    let task_loss = ids.task_loss.as_deref();
    let output = ids.output.as_deref();
    let inference_student = ids.inference_student.as_deref();
    let allowed_pairs = teacher_student_edge_pairs(
        input,
        teacher,
        student,
        residual,
        task_loss,
        output,
        inference_student,
    );

    plan.edges.retain(|edge| {
        if edge.from == edge.to {
            return false;
        }
        allowed_pairs
            .iter()
            .any(|(from, to)| edge.from == *from && edge.to == *to)
    });

    if let (Some(input), Some(teacher)) = (input, teacher) {
        ensure_edge(
            &mut plan.edges,
            "e_ts_input_to_teacher",
            input,
            teacher,
            "",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        );
    }
    if let Some(input) = input {
        ensure_edge(
            &mut plan.edges,
            "e_ts_input_to_student",
            input,
            student,
            "",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        );
    }
    if let (Some(teacher), Some(residual)) = (teacher, residual) {
        ensure_edge(
            &mut plan.edges,
            "e_ts_teacher_to_residual",
            teacher,
            residual,
            "",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        );
    }
    if let Some(teacher) = teacher {
        ensure_edge(
            &mut plan.edges,
            "e_ts_teacher_to_student",
            teacher,
            student,
            "",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Main,
        );
    }
    if let Some(residual) = residual {
        ensure_edge(
            &mut plan.edges,
            "e_ts_student_to_residual",
            student,
            residual,
            "",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        );
    }
    if let Some(task_loss) = task_loss {
        ensure_edge(
            &mut plan.edges,
            "e_ts_student_to_loss",
            student,
            task_loss,
            "",
            EdgeSemantic::Loss,
            EdgeStyle::Solid,
            EdgeImportance::Normal,
        );
    }
    if let (Some(residual), Some(task_loss)) = (residual, task_loss) {
        ensure_edge(
            &mut plan.edges,
            "e_ts_residual_to_loss",
            residual,
            task_loss,
            "",
            EdgeSemantic::Supervision,
            EdgeStyle::Dash,
            EdgeImportance::Main,
        );
    }
    if let (Some(inference_student), Some(output)) = (inference_student, output) {
        ensure_edge(
            &mut plan.edges,
            "e_ts_inference_student_to_output",
            inference_student,
            output,
            "",
            EdgeSemantic::DataFlow,
            EdgeStyle::Solid,
            EdgeImportance::Main,
        );
    }

    for edge in &mut plan.edges {
        if edge.from == student && Some(edge.to.as_str()) == residual {
            edge.semantic = EdgeSemantic::DataFlow;
            edge.style = EdgeStyle::Solid;
            edge.importance = EdgeImportance::Normal;
            edge.label.clear();
        }
        if Some(edge.from.as_str()) == residual && Some(edge.to.as_str()) == task_loss {
            edge.semantic = EdgeSemantic::Supervision;
            edge.style = EdgeStyle::Dash;
            edge.importance = EdgeImportance::Main;
            edge.label.clear();
        }
    }
}

fn teacher_student_edge_pairs<'a>(
    input: Option<&'a str>,
    teacher: Option<&'a str>,
    student: &'a str,
    residual: Option<&'a str>,
    task_loss: Option<&'a str>,
    output: Option<&'a str>,
    inference_student: Option<&'a str>,
) -> Vec<(&'a str, &'a str)> {
    let mut pairs = Vec::new();
    push_pair(&mut pairs, input, teacher);
    push_pair(&mut pairs, input, Some(student));
    push_pair(&mut pairs, teacher, residual);
    push_pair(&mut pairs, teacher, Some(student));
    push_pair(&mut pairs, Some(student), residual);
    push_pair(&mut pairs, residual, task_loss);
    push_pair(&mut pairs, Some(student), task_loss);
    push_pair(&mut pairs, inference_student, output);
    pairs
}

fn push_pair<'a>(pairs: &mut Vec<(&'a str, &'a str)>, from: Option<&'a str>, to: Option<&'a str>) {
    if let (Some(from), Some(to)) = (from, to) {
        pairs.push((from, to));
    }
}

fn canonicalize_teacher_student_annotations(plan: &mut FigurePlan) {
    plan.annotations.clear();
}

fn prune_unused_teacher_student_regions(plan: &mut FigurePlan) {
    plan.layout.regions.retain(|region| {
        plan.components
            .iter()
            .any(|component| component.region == region.id)
    });
}

fn ensure_edge(
    edges: &mut Vec<Edge>,
    id: &str,
    from: &str,
    to: &str,
    label: &str,
    semantic: EdgeSemantic,
    style: EdgeStyle,
    importance: EdgeImportance,
) {
    if let Some(edge) = edges
        .iter_mut()
        .find(|edge| edge.from == from && edge.to == to)
    {
        edge.semantic = semantic;
        edge.style = style;
        edge.importance = importance;
        if edge.label.trim().is_empty() {
            edge.label = label.to_string();
        }
        return;
    }

    let mut edge_id = id.to_string();
    if edges.iter().any(|edge| edge.id == edge_id) {
        let mut suffix = 2;
        while edges.iter().any(|edge| edge.id == format!("{id}_{suffix}")) {
            suffix += 1;
        }
        edge_id = format!("{id}_{suffix}");
    }
    edges.push(Edge::new(
        &edge_id, from, to, label, semantic, style, importance,
    ));
}

fn find_component(plan: &FigurePlan, predicate: impl Fn(&Component) -> bool) -> Option<String> {
    plan.components
        .iter()
        .find(|component| predicate(component))
        .map(|component| component.id.clone())
}

fn contains_any(component: &Component, needles: &[&str]) -> bool {
    let text = format!("{} {}", component.id, component.label).to_lowercase();
    needles.iter().any(|needle| text.contains(needle))
}

fn semantic_module_candidate(component: &Component) -> bool {
    !contains_any(
        component,
        &[
            "title",
            "overview",
            "header",
            "training label",
            "inference label",
        ],
    )
}

fn shorten_label_if_needed(component: &mut Component, replacement: &str) {
    let too_long = component.label.chars().count() > 26 || component.label.lines().count() > 2;
    if too_long {
        component.label = replacement.to_string();
    }
}

fn unique_id(plan: &FigurePlan, base: &str, exists: impl Fn(&FigurePlan, &str) -> bool) -> String {
    if !exists(plan, base) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !exists(plan, &candidate) {
            return candidate;
        }
        suffix += 1;
    }
}
