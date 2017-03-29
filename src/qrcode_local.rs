//! Render QR code to String, on the basis of qrcode crate

use image::Luma;
use qrcode::QrCode;
use qrcode::render::{Renderer, BlankAndWhitePixel};

use Result;

/// Counterfeit of `qrcode::render::Renderer`
///
/// A QR code renderer. This is a builder type which converts a bool-vector into
/// an image.
pub struct RendererLocal<'a, P: BlankAndWhitePixel> {
    content: &'a [bool],
    modules_count: u32, // <- we call it `modules_count` here to avoid ambiguity of `width`.
    quiet_zone: u32,
    module_size: u32,

    dark_color: P,
    light_color: P,
    has_quiet_zone: bool,
}

impl<'a, P: BlankAndWhitePixel + 'static> RendererLocal<'a, P> {
    /// Convert `qrcode::render::Renderer` to Self
    fn from_mccoy(mccoy: &Renderer<'a, P>) -> &'a RendererLocal<'a, P> {
        unsafe { &*(mccoy as *const Renderer<P> as *const RendererLocal<P>) }
    }

    /// Renders the QR code into String.
    pub fn to_string(&self, on_str: &str, off_str: &str) -> String {
        let w = self.modules_count;
        let qz = if self.has_quiet_zone {
            self.quiet_zone
        } else {
            0
        };
        let width = w + 2 * qz;

        let mut str = String::new();
        let mut i = 0;
        for y in 0..width {
            for x in 0..width {
                if qz <= x && x < w + qz && qz <= y && y < w + qz {
                    if self.content[i] {
                        str += on_str;
                    } else {
                        str += off_str;
                    };
                    i += 1;
                } else {
                    str += off_str;
                };
            }
            str.push('\n')
        }
        str
    }
}

pub fn render_and_print_qr_code<T: AsRef<[u8]>>(input: T) -> Result<()> {
    let code = QrCode::new(input)?;
    let render = code.render();
    let render: &RendererLocal<Luma<u8>> = RendererLocal::from_mccoy(&render);

    let s = render.to_string("\x1b[49m  \x1b[0m", "\x1b[7m  \x1b[0m");
    println!("{}", s);
    Ok(())
}
