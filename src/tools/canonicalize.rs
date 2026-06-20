use crate::schema::{FigurePlan, ImageProviderKind};

pub fn canonicalize_plan_for_render(plan: &mut FigurePlan, image_provider: ImageProviderKind) {
    if matches!(image_provider, ImageProviderKind::None) {
        strip_image_asset_expectations(plan);
    }
}

fn strip_image_asset_expectations(plan: &mut FigurePlan) {
    plan.assets.clear();
    for component in &mut plan.components {
        component.allowed_asset_id = None;
    }
}
