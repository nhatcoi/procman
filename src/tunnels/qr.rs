use qrcode::render::unicode;
use qrcode::QrCode;

pub fn render_qr(url: &str) -> Option<String> {
    let code = QrCode::new(url.as_bytes()).ok()?;
    let image = code
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Dark)
        .light_color(unicode::Dense1x2::Light)
        .build();
    Some(image)
}
