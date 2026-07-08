//! QR rendering for the qr-event channel (design doc `05 §4`): a room
//! full of phones, one slide, every scan attributed. SVG out — pure text,
//! no raster dependencies, byte-identical per input.

use crate::package::SharePackage;

/// Errors from QR encoding (input too long for the symbol capacity).
#[derive(Debug)]
pub struct QrError(pub String);

impl std::fmt::Display for QrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "qr: {}", self.0)
    }
}

impl std::error::Error for QrError {}

/// Render the package's URL as an SVG QR code (medium error correction —
/// survives projector glare and phone-angle abuse). `border` is quiet-zone
/// modules; 4 is the spec minimum.
pub fn qr_svg(p: &SharePackage, border: u32) -> Result<String, QrError> {
    let code = qrcodegen::QrCode::encode_text(&p.url, qrcodegen::QrCodeEcc::Medium)
        .map_err(|e| QrError(e.to_string()))?;
    let size = u32::try_from(code.size()).map_err(|e| QrError(e.to_string()))?;
    let dim = size + border * 2;
    let mut path = String::new();
    for y in 0..size {
        for x in 0..size {
            #[allow(clippy::cast_possible_wrap)] // size ≤ 177 per the QR spec
            if code.get_module(x as i32, y as i32) {
                use std::fmt::Write as _;
                let _ = write!(path, "M{},{}h1v1h-1z", x + border, y + border);
            }
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {dim} {dim}\" stroke=\"none\">\
         <rect width=\"100%\" height=\"100%\" fill=\"#FFFFFF\"/>\
         <path d=\"{path}\" fill=\"#000000\"/></svg>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package() -> SharePackage {
        SharePackage {
            url: "https://wgl.example/7Kp2mQ9x".into(),
            title: String::new(),
            description: String::new(),
            image_url: None,
            token: "7Kp2mQ9x".into(),
        }
    }

    #[test]
    fn qr_svg_is_valid_and_pure() {
        let svg = qr_svg(&package(), 4).unwrap();
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.contains("<path d=\"M"));
        assert!(svg.ends_with("</svg>"));
        // Purity: byte-identical on re-render (the CP-8 gate).
        assert_eq!(svg, qr_svg(&package(), 4).unwrap());
        // Different borders change geometry, deterministically.
        assert_ne!(svg, qr_svg(&package(), 2).unwrap());
    }

    #[test]
    fn oversized_payload_errs_politely() {
        let mut p = package();
        p.url = "x".repeat(8000); // beyond any QR version's capacity
        assert!(qr_svg(&p, 4).is_err());
    }
}
