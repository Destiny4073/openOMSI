//! Roboto, the interface font (Apache 2.0), in several weights of its variable font.
//! What Roboto has not (the CJK scripts the interface speaks) comes from a system font.

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, VariableFont};

const ROBOTO: &[u8] = include_bytes!("../../../assets/fonts/Roboto-VariableFont_wdth,wght.ttf");

/// System fonts with CJK glyphs, tried in order (the first that opens wins). A box shows
/// for a CJK character when none of them exists.
const CJK_FONTS: &[&str] = &[
    // Windows
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/msyh.ttf",
    "C:/Windows/Fonts/simhei.ttf",
    "C:/Windows/Fonts/simsun.ttc",
    // macOS
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    // Linux (Debian, Fedora, Arch names)
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
    // Android
    "/system/fonts/NotoSansCJK-Regular.ttc",
    "/system/fonts/NotoSansSC-Regular.otf",
];

static CJK: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();

/// The bytes of the first system font with CJK glyphs, or none (shared with the game's own
/// text renderer, which draws over the picture).
pub fn cjk_bytes() -> Option<&'static Vec<u8>> {
    CJK.get_or_init(|| {
        for path in CJK_FONTS {
            if let Ok(bytes) = std::fs::read(path) {
                if FontVec::try_from_vec_and_index(bytes.clone(), 0).is_ok() {
                    log::info!("interface: CJK glyphs from {path}");
                    return Some(bytes);
                }
            }
        }
        log::info!("interface: no system font with CJK glyphs found");
        None
    })
    .as_ref()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Weight {
    Regular,
    Medium,
    Bold,
    Black,
    /// Bold and narrow (Roboto's `wdth` 75): figures in a tight space.
    Condensed,
}

impl Weight {
    const ALL: [Weight; 5] = [Weight::Regular, Weight::Medium, Weight::Bold, Weight::Black, Weight::Condensed];
    fn axes(self) -> (f32, f32) {
        match self {
            Weight::Regular => (400.0, 100.0),
            Weight::Medium => (500.0, 100.0),
            Weight::Bold => (700.0, 100.0),
            Weight::Black => (900.0, 100.0),
            Weight::Condensed => (700.0, 75.0),
        }
    }
}

/// A line of text as coverage: `w` x `h` alpha values; the baseline is `ascent` pixels
/// below the top.
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub w: u32,
    pub h: u32,
    pub alpha: Vec<u8>,
    pub ascent: f32,
}

/// A character Roboto has in place of one it has not (a box would show).
fn substitute(c: char) -> char {
    match c {
        '→' | '▸' | '➜' | '⟶' | '►' | '▶' => '›',
        '←' | '◂' | '◀' => '‹',
        '★' | '☆' => '•',
        '⚠' => '!',
        '✓' | '✔' => '•',
        '✕' | '✖' => '×',
        c => c,
    }
}

/// Letters written as a base and a combining mark (Unicode's decomposed form, which is how
/// macOS stores file names: "Eiseska\u{308}lte.owt") as the one letter Roboto draws - the
/// mark alone was a box after a plain "a" in the launcher's weather list.
fn composed(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.chars().any(|c| ('\u{300}'..='\u{36f}').contains(&c)) {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if ('\u{300}'..='\u{36f}').contains(&c) {
            if let Some(base) = out.pop() {
                match compose(base, c) {
                    Some(k) => out.push(k),
                    None => out.push(base),
                }
            }
            continue;
        }
        out.push(c);
    }
    std::borrow::Cow::Owned(out)
}

/// The precomposed letter for `base` + combining `mark` (the Latin letters of the maps'
/// countries: German, French, Polish, Czech, Hungarian, Serbian/Croatian latin).
fn compose(base: char, mark: char) -> Option<char> {
    let table: &[(char, &str, &str)] = &[
        ('\u{308}', "aeiouyAEIOUY", "äëïöüÿÄËÏÖÜŸ"),
        ('\u{301}', "aeiouyAEIOUYcnszlrCNSZLR", "áéíóúýÁÉÍÓÚÝćńśźĺŕĆŃŚŹĹŔ"),
        ('\u{300}', "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
        ('\u{302}', "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ('\u{303}', "anoANO", "ãñõÃÑÕ"),
        ('\u{30c}', "cdenrstzCDENRSTZ", "čďěňřšťžČĎĚŇŘŠŤŽ"),
        ('\u{30a}', "auAU", "åůÅŮ"),
        ('\u{327}', "cstCST", "çşţÇŞŢ"),
        ('\u{328}', "aeAE", "ąęĄĘ"),
        ('\u{307}', "zZ", "żŻ"),
        ('\u{30b}', "ouOU", "őűŐŰ"),
    ];
    let (_, from, to) = table.iter().find(|(m, _, _)| *m == mark)?;
    let k = from.chars().position(|c| c == base)?;
    to.chars().nth(k)
}

/// Padding around a rendered line (pixels), so that linear filtering does not bleed.
pub const PAD: u32 = 1;

pub struct Fonts {
    faces: Vec<(Weight, FontVec)>,
    /// A system font with the CJK glyphs Roboto has not.
    cjk: Option<FontVec>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        let faces = Weight::ALL
            .iter()
            .filter_map(|&w| {
                let mut f = FontVec::try_from_vec(ROBOTO.to_vec()).ok()?;
                let (wght, wdth) = w.axes();
                f.set_variation(b"wght", wght);
                f.set_variation(b"wdth", wdth);
                Some((w, f))
            })
            .collect();
        Fonts { faces, cjk: cjk_bytes().and_then(|b| FontVec::try_from_vec_and_index(b.clone(), 0).ok()) }
    }

    fn face(&self, w: Weight) -> &FontVec {
        &self.faces.iter().find(|(k, _)| *k == w).unwrap_or(&self.faces[0]).1
    }

    /// The face that draws `c` at `w`: Roboto, or the CJK font for one it has not.
    fn face_for(&self, w: Weight, c: char) -> &FontVec {
        let f = self.face(w);
        if f.glyph_id(c).0 == 0 {
            if let Some(fb) = &self.cjk {
                if fb.glyph_id(c).0 != 0 {
                    return fb;
                }
            }
        }
        f
    }

    /// Width of `text` in pixels at `px`.
    pub fn width(&self, text: &str, px: f32, weight: Weight) -> f32 {
        let translated = crate::i18n::tr(text);
        let comp = composed(&translated);
        let text = &*comp;
        let scale = PxScale::from(px);
        let mut w = 0.0;
        let mut prev: Option<(&FontVec, ab_glyph::GlyphId)> = None;
        for c in text.chars().map(substitute) {
            let face = self.face_for(weight, c);
            let f = face.as_scaled(scale);
            let id = f.glyph_id(c);
            if let Some((pf, pid)) = prev {
                // (kerning only makes sense within one face)
                if std::ptr::eq(pf, face) {
                    w += f.kern(pid, id);
                }
            }
            w += f.h_advance(id);
            prev = Some((face, id));
        }
        w
    }

    /// Line height (ascent − descent) at `px`.
    pub fn line_height(&self, px: f32, weight: Weight) -> f32 {
        let f = self.face(weight).as_scaled(PxScale::from(px));
        f.ascent() - f.descent()
    }

    /// Height of capitals above the baseline at `px` (for centring a line on a box).
    pub fn cap_height(&self, px: f32, weight: Weight) -> f32 {
        let f = self.face(weight).as_scaled(PxScale::from(px));
        let id = f.glyph_id('H');
        let g = id.with_scale(PxScale::from(px));
        self.face(weight).outline_glyph(g).map(|o| -o.px_bounds().min.y).unwrap_or(f.ascent() * 0.7)
    }

    /// The longest start of `text` that fits in `max` pixels, with an ellipsis when cut.
    pub fn fit(&self, text: &str, px: f32, weight: Weight, max: f32) -> String {
        let translated = crate::i18n::tr(text);
        let comp = composed(&translated);
        let text = &*comp;
        if self.width(text, px, weight) <= max {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let s: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
            if self.width(&s, px, weight) <= max {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        chars[..lo].iter().collect::<String>().trim_end().to_string() + "…"
    }

    /// Rasterise one line.
    pub fn render(&self, text: &str, px: f32, weight: Weight) -> Bitmap {
        let comp = composed(text);
        let text = &*comp;
        let scale = PxScale::from(px);
        let scaled = self.face(weight).as_scaled(scale);
        let chars: Vec<char> = text.chars().map(substitute).collect();
        // when the line needs the CJK font, the canvas covers its taller extents too
        let uses_cjk = self
            .cjk
            .as_ref()
            .is_some_and(|fb| chars.iter().any(|&c| scaled.glyph_id(c).0 == 0 && fb.glyph_id(c).0 != 0));
        let (asc, desc) = match self.cjk.as_ref().filter(|_| uses_cjk) {
            Some(fb) => {
                let s = fb.as_scaled(scale);
                (scaled.ascent().max(s.ascent()), scaled.descent().min(s.descent()))
            }
            None => (scaled.ascent(), scaled.descent()),
        };
        let pad = PAD as f32;
        let h = ((asc - desc).ceil() as u32 + 2 * PAD).max(1);
        let mut glyphs: Vec<(&FontVec, ab_glyph::Glyph)> = Vec::new();
        let mut x = pad;
        let mut prev: Option<(&FontVec, ab_glyph::GlyphId)> = None;
        for c in chars {
            let face = self.face_for(weight, c);
            let f = face.as_scaled(scale);
            let id = f.glyph_id(c);
            if let Some((pf, pid)) = prev {
                // (kerning only makes sense within one face)
                if std::ptr::eq(pf, face) {
                    x += f.kern(pid, id);
                }
            }
            glyphs.push((face, id.with_scale_and_position(scale, ab_glyph::point(x, pad + asc))));
            x += f.h_advance(id);
            prev = Some((face, id));
        }
        let w = (x.ceil() as u32 + PAD).max(1);
        let mut cov = vec![0f32; (w * h) as usize];
        for (face, g) in glyphs {
            if let Some(o) = face.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, c| {
                    let xx = b.min.x as i32 + gx as i32;
                    let yy = b.min.y as i32 + gy as i32;
                    if xx >= 0 && yy >= 0 && (xx as u32) < w && (yy as u32) < h {
                        let i = (yy as u32 * w + xx as u32) as usize;
                        cov[i] = (cov[i] + c).min(1.0);
                    }
                });
            }
        }
        Bitmap { w, h, alpha: cov.iter().map(|c| (c * 255.0).round() as u8).collect(), ascent: asc + pad }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_differ_and_text_fits() {
        let f = Fonts::new();
        let regular = f.render("Bauernhof", 20.0, Weight::Regular);
        let bold = f.render("Bauernhof", 20.0, Weight::Bold);
        let ink = |b: &Bitmap| b.alpha.iter().map(|&a| a as u64).sum::<u64>();
        assert!(ink(&bold) > ink(&regular) * 11 / 10, "bold {} regular {}", ink(&bold), ink(&regular));
        assert!(f.width("Bauernhof", 20.0, Weight::Condensed) < f.width("Bauernhof", 20.0, Weight::Bold));
        let cut = f.fit("Krankenhaus Grundorf Nord", 16.0, Weight::Regular, 100.0);
        assert!(cut.ends_with('…') && f.width(&cut, 16.0, Weight::Regular) <= 100.0, "{cut}");
        assert_eq!(f.fit("Kurz", 16.0, Weight::Regular, 100.0), "Kurz");
    }
}

#[cfg(test)]
mod glyph_tests {
    use super::*;
    #[test]
    fn missing_symbols_are_substituted() {
        let f = ab_glyph::FontRef::try_from_slice(ROBOTO).unwrap();
        for c in "→★⚠✓▸ Bauernhof · 12 °C - ДёЖ".chars() {
            assert!(f.glyph_id(substitute(c)).0 != 0 || substitute(c) == ' ', "{c}");
        }
    }

    #[test]
    fn cjk_text_renders_ink() {
        let f = Fonts::new();
        let b = f.render("公交车 简体中文", 20.0, Weight::Regular);
        if f.cjk.is_none() {
            return; // no system font with CJK glyphs on this machine
        }
        assert!(b.alpha.iter().any(|&a| a > 0), "the CJK line drew nothing");
        assert!(f.width("公交车", 20.0, Weight::Regular) > 20.0, "CJK advances look wrong");
    }
}
