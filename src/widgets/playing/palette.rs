//! Dynamic accent colour and blurred background generated from the current cover.
use crate::{application::Application, models::CurrentState};
use gtk::{gdk, glib, prelude::*};

/// Keep the dynamic CSS in sync with the current song's cover. Call once per login.
pub fn watch(state: &CurrentState) {
    let st = state.clone();
    super::follow_song(state, state, move |_, song| {
        if let Some(p) = song.gdkPaintable() {
            update(&st, &p);
        }
    });
}

fn rgb(c: &color_thief::Color) -> String {
    format!("{},{},{}", c.r, c.g, c.b)
}

/// Port of `update_palette`: accent from the dominant colour, background from a tiny blurred cover.
pub fn update(state: &CurrentState, paintable: &gdk::Paintable) {
    let Some(tex) = paintable.downcast_ref::<gdk::Texture>() else { return };
    let mut dl = gdk::TextureDownloader::new(tex);
    dl.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = dl.download_bytes();
    let (w, h) = (tex.width() as usize, tex.height() as usize);
    let pixels: Vec<u8> = (0..h).flat_map(|y| bytes[y * stride..y * stride + w * 4].iter().copied()).collect();
    let Ok(palette) = color_thief::get_palette(&pixels, color_thief::ColorFormat::Rgba, 10, 2) else { return };
    let (Some(c0), c1) = (palette.first(), palette.get(1).or(palette.first())) else { return };
    let c1 = c1.unwrap_or(c0);

    let blur = image::RgbaImage::from_raw(w as u32, h as u32, pixels).map(|img| {
        let small = image::imageops::resize(&img, 24, 24, image::imageops::FilterType::Triangle);
        let mut blurred = image::imageops::blur(&small, 2.0);
        blurred.pixels_mut().for_each(|p| p.0[3] = (255.0 * 0.3) as u8);
        let mut png = std::io::Cursor::new(Vec::new());
        let _ = blurred.write_to(&mut png, image::ImageFormat::Png);
        glib::base64_encode(png.get_ref()).to_string()
    });
    let blur = blur.unwrap_or_default();

    let css = format!(
        r#"
        window.dynamic-accent {{
            --accent-color: oklab(from rgb({c0}) var(--standalone-color-oklab));
        }}
        window.popout-window.dynamic-bg-blur,
        window.dynamic-bg-blur bottom-sheet#main-bottom-sheet sheet > stack,
        window.dynamic-bg-blur:not(.global-dynamic-bg-gradient):not(.global-dynamic-bg-blur) overlay-split-view .main_sidebar,
        window.global-dynamic-bg-blur {{
            background-image: url("data:image/png;base64,{blur}");
        }}
        window.popout-window.dynamic-bg-gradient,
        window.dynamic-bg-gradient bottom-sheet#main-bottom-sheet sheet > stack,
        window.dynamic-bg-gradient:not(.global-dynamic-bg-gradient):not(.global-dynamic-bg-blur) overlay-split-view .main_sidebar,
        window.global-dynamic-bg-gradient {{
            background-image: linear-gradient(to bottom, rgba({c0},0.4), rgba({c1},0.4));
        }}
        "#,
        c0 = rgb(c0),
        c1 = rgb(c1),
    );
    Application::get().css_provider().load_from_string(&css);
    state.set_accent_color(Some((c0.r, c0.g, c0.b)));
}
