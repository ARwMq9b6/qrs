//! Render QR code to String, on the basis of qrcode crate

use std::result;
use std::cell::RefCell;

use term;
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

    /// Walk the render,
    /// match
    ///     on point    => exec `on_fn`,
    ///     off point   => exec `off_fn`,
    ///     ln          => exec `ln_fn`
    pub fn walk<E, On, Off, Ln>(&self, on_fn: On, off_fn: Off, ln_fn: Ln) -> result::Result<(), E>
        where On: Fn() -> result::Result<(), E>,
              Off: Fn() -> result::Result<(), E>,
              Ln: Fn() -> result::Result<(), E>
    {
        let w = self.modules_count;
        let qz = if self.has_quiet_zone {
            self.quiet_zone
        } else {
            0
        };
        let width = w + 2 * qz;

        let mut i = 0;
        for y in 0..width {
            for x in 0..width {
                if qz <= x && x < w + qz && qz <= y && y < w + qz {
                    if self.content[i] {
                        on_fn()?
                    } else {
                        off_fn()?
                    };
                    i += 1;
                } else {
                    off_fn()?
                };
            }
            ln_fn()?
        }
        Ok(())
    }
}

pub fn render_and_print_qr_code<T: AsRef<[u8]>>(input: T) -> Result<()> {
    let code = QrCode::new(input)?;
    let render = code.render();
    let render: &RendererLocal<Luma<u8>> = RendererLocal::from_mccoy(&render);

    let rc = RefCell::new(term::stdout().ok_or("term::stdout() err")?);
    let on_fn = || -> Result<()> {
        let mut t = rc.borrow_mut();
        t.bg(term::color::BLACK)?;
        Ok(write!(t, "  ")?)
    };
    let off_fn = || -> Result<()> {
        let mut t = rc.borrow_mut();
        t.bg(term::color::WHITE)?;
        Ok(write!(t, "  ")?)
    };
    let ln_fn = || -> Result<()> {
        let mut t = rc.borrow_mut();
        t.reset();
        Ok(write!(t, "\n")?)
    };

    Ok(render.walk(on_fn, off_fn, ln_fn)?)
}
