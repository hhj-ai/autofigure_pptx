use methodfig::schema::{Canvas, CanvasAspect, DrawLabel, DrawObject, DrawPlan};
use methodfig::tools::draw_plan::{draw_plan_material_changes, has_material_draw_plan_change};

#[test]
fn material_diff_detects_bbox_points_style_and_text_changes() {
    let before = minimal_plan(vec![
        DrawObject::Box {
            id: "student".to_string(),
            bbox: [0.20, 0.30, 0.42, 0.48],
            text: "Student".to_string(),
            role: "main".to_string(),
            style: "primary_module".to_string(),
            z: 20,
        },
        DrawObject::Connector {
            id: "teacher_to_student".to_string(),
            points: vec![[0.10, 0.40], [0.20, 0.40]],
            from: Some("teacher".to_string()),
            to: Some("student".to_string()),
            style: "supervision_dash".to_string(),
            label: Some(DrawLabel {
                text: "distill".to_string(),
                bbox: [0.12, 0.34, 0.20, 0.38],
            }),
            z: 10,
        },
    ]);
    let mut after = before.clone();
    if let DrawObject::Box { bbox, .. } = &mut after.objects[0] {
        *bbox = [0.25, 0.30, 0.47, 0.48];
    }
    if let DrawObject::Connector { points, label, .. } = &mut after.objects[1] {
        points.insert(1, [0.20, 0.30]);
        label.as_mut().unwrap().bbox = [0.22, 0.28, 0.30, 0.32];
    }

    let changes = draw_plan_material_changes(&before, &after);

    assert!(has_material_draw_plan_change(&before, &after));
    assert!(changes.iter().any(|change| change.contains("student bbox")));
    assert!(changes
        .iter()
        .any(|change| change.contains("teacher_to_student points")));
    assert!(changes
        .iter()
        .any(|change| change.contains("teacher_to_student label bbox")));
}

#[test]
fn material_diff_ignores_equivalent_serialization_noise() {
    let before = minimal_plan(vec![DrawObject::Text {
        id: "note".to_string(),
        bbox: [0.10, 0.10, 0.30, 0.16],
        text: "same".to_string(),
        style: "annotation".to_string(),
        z: 30,
    }]);
    let after = before.clone();

    assert!(!has_material_draw_plan_change(&before, &after));
    assert!(draw_plan_material_changes(&before, &after).is_empty());
}

fn minimal_plan(objects: Vec<DrawObject>) -> DrawPlan {
    DrawPlan {
        version: "0.2".to_string(),
        canvas: Canvas {
            aspect: CanvasAspect::PaperWide,
            target_width_mm: 85,
            safe_margin: 0.06,
        },
        style_tokens: Default::default(),
        objects,
    }
}
