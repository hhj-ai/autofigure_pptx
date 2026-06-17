use methodfig::tools::doctor::font_names_to_check;

#[test]
fn doctor_checks_wps_friendly_cjk_and_latin_fonts() {
    let fonts = font_names_to_check();
    assert!(fonts.contains(&"Microsoft YaHei"));
    assert!(fonts.contains(&"DengXian"));
    assert!(fonts.contains(&"SimHei"));
    assert!(fonts.contains(&"SimSun"));
    assert!(fonts.contains(&"Arial"));
}
