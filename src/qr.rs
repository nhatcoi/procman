use qrcode::render::unicode;
use qrcode::QrCode;

pub fn render_qr(text: &str) -> Option<String> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .quiet_zone(true)
        .build();
    Some(image)
}
