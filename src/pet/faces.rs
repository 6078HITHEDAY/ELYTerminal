//! Ely's faces: the icon's head, cut at the edge her fingers hold, one SVG a face.

use std::sync::Arc;

use gpui::{Image, ImageFormat};

/// What Ely's face shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Calm,
    Blink,
    /// Eyes on the terminal below her, as while you type or a program works.
    Down,
    Left,
    Right,
    /// A program finished: eyes shut in a smile, and sparkles.
    Happy,
    /// A program waits on you: wide eyes, an open mouth, and a mark.
    Surprised,
    /// A program failed: raised brows and a drop.
    Worried,
    Sleepy,
    /// Pressed: a wink and a heart.
    Petted,
}

impl Face {
    pub const ALL: [Face; 10] = [
        Face::Calm,
        Face::Blink,
        Face::Down,
        Face::Left,
        Face::Right,
        Face::Happy,
        Face::Surprised,
        Face::Worried,
        Face::Sleepy,
        Face::Petted,
    ];
}

/// The head's box in the icon's units; the edge she holds runs along its bottom.
const VIEW: [u32; 4] = [110, 6, 292, 280];

/// Pixels a unit draws at: three, so a Retina screen draws her sharp.
const SCALE: u32 = 3;

/// Every face as an image gpui draws once and keeps.
pub fn images() -> Vec<(Face, Arc<Image>)> {
    Face::ALL
        .into_iter()
        .map(|face| {
            let svg = svg(face).into_bytes();
            (face, Arc::new(Image::from_bytes(ImageFormat::Svg, svg)))
        })
        .collect()
}

/// The SVG of `face`.
pub fn svg(face: Face) -> String {
    let (eyes, mouth, mark) = match face {
        Face::Calm => (open((0, 0), 1.0), SMILE, ""),
        Face::Blink => (SHUT.into(), SMILE, ""),
        Face::Down => (open((0, 5), 1.0), FLAT, ""),
        Face::Left => (open((-5, 0), 1.0), SMILE, ""),
        Face::Right => (open((5, 0), 1.0), SMILE, ""),
        Face::Happy => (ARCS.into(), GRIN, SPARKLES),
        Face::Surprised => (open((0, 0), 1.12), GASP, BANG),
        Face::Worried => (open((0, 3), 1.0), FROWN, DROP),
        Face::Sleepy => (LINES.into(), FLAT, SNORE),
        Face::Petted => (wink(), GRIN, HEART),
    };
    let blush = if face == Face::Petted { FLUSH } else { BLUSH };
    let brows = if face == Face::Worried { BROWS } else { "" };
    let [x, y, w, h] = VIEW;
    let (width, height) = (w * SCALE, h * SCALE);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{x} {y} {w} {h}">{HEAD}{eyes}{blush}{mouth}{HAIR}{brows}{HANDS}{mark}</svg>"#
    )
}

/// An eye at `cx`, its iris moved by `look`, the whole eye grown by `wide`.
fn eye(cx: i32, (dx, dy): (i32, i32), wide: f32) -> String {
    format!(
        r##"<g transform="translate({cx} 188) scale({wide}) translate({} -188)"><ellipse cx="{cx}" cy="188" rx="18" ry="23" fill="#A24FB9"/><g transform="translate({dx} {dy})"><ellipse cx="{cx}" cy="198" rx="13.5" ry="11.5" fill="#EE84C4"/><ellipse cx="{cx}" cy="190" rx="7.5" ry="10.5" fill="#5A2368"/><circle cx="{}" cy="178" r="7" fill="#FFFFFF"/><circle cx="{}" cy="199" r="3.2" fill="#FFFFFF"/></g></g>"##,
        -cx,
        cx - 7,
        cx + 7
    )
}

/// Both eyes open, with their lids and lashes.
fn open(look: (i32, i32), wide: f32) -> String {
    format!(
        "{}{}{LEFT_LID}{RIGHT_LID}{LASHES}",
        eye(222, look, wide),
        eye(290, look, wide)
    )
}

/// The left eye shut in a smile, the right one open.
fn wink() -> String {
    format!("{}{RIGHT_LID}{LEFT_ARC}", eye(290, (0, 0), 1.0))
}

const LEFT_LID: &str = r##"<path fill="#45233F" d="M194 182C195 168 208 159 224 159C234 159 241 164 244 172L240 173C236 168 231 165 224 165C214 165 207 172 204 183Z"/>"##;
const RIGHT_LID: &str = r##"<path fill="#45233F" d="M318 182C317 168 304 159 288 159C278 159 271 164 268 172L272 173C276 168 281 165 288 165C298 165 305 172 308 183Z"/>"##;
const LASHES: &str = r##"<path fill="none" stroke="#C2506F" stroke-width="2" stroke-linecap="round" opacity=".35" d="M215 212Q222 214 229 212M283 212Q290 214 297 212"/>"##;
const LEFT_ARC: &str = r##"<path fill="none" stroke="#45233F" stroke-width="7" stroke-linecap="round" d="M205 197Q222 176 239 197"/>"##;
const SHUT: &str = r##"<g fill="none" stroke="#45233F" stroke-width="6" stroke-linecap="round"><path d="M205 192Q222 203 239 192"/><path d="M273 192Q290 203 307 192"/></g>"##;
const ARCS: &str = r##"<g fill="none" stroke="#45233F" stroke-width="7" stroke-linecap="round"><path d="M205 197Q222 176 239 197"/><path d="M273 197Q290 176 307 197"/></g>"##;
const LINES: &str = r##"<g fill="none" stroke="#45233F" stroke-width="6" stroke-linecap="round"><path d="M205 194H239"/><path d="M273 194H307"/></g>"##;
const BROWS: &str = r##"<g fill="none" stroke="#45233F" stroke-width="5" stroke-linecap="round"><path d="M206 160L234 152"/><path d="M306 160L278 152"/></g>"##;
const SMILE: &str = r##"<path fill="none" stroke="#C2506F" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" d="M244 222Q250 229 256 223Q262 229 268 222"/>"##;
const GRIN: &str = r##"<path fill="#C2506F" d="M243 219Q256 241 269 219Z"/><path fill="#FF8DAA" d="M249 229Q256 236 263 229Q256 225 249 229Z"/>"##;
const GASP: &str = r##"<ellipse cx="256" cy="226" rx="5.5" ry="7" fill="#C2506F"/>"##;
const FLAT: &str = r##"<path fill="none" stroke="#C2506F" stroke-width="3.5" stroke-linecap="round" d="M249 225H263"/>"##;
const FROWN: &str = r##"<path fill="none" stroke="#C2506F" stroke-width="3.5" stroke-linecap="round" d="M246 228Q256 220 266 228"/>"##;
const BLUSH: &str = r##"<g opacity=".55" fill="#FF8DAA"><ellipse cx="198" cy="216" rx="15" ry="8"/><ellipse cx="314" cy="216" rx="15" ry="8"/></g>"##;
const FLUSH: &str = r##"<g opacity=".85" fill="#FF7A9C"><ellipse cx="198" cy="216" rx="17" ry="9"/><ellipse cx="314" cy="216" rx="17" ry="9"/></g>"##;
const BANG: &str = r##"<g fill="#FFB020" stroke="#45233F" stroke-width="3"><path d="M371 38L385 38L380 86L376 86Z"/><circle cx="378" cy="100" r="7"/></g>"##;
const SNORE: &str = r##"<g fill="none" stroke="#9FA3D9" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"><path d="M352 70H372L352 92H372"/><path d="M380 38H394L380 54H394"/></g>"##;
const HEART: &str = r##"<path fill="#FF5C93" d="M378 70C366 56 350 66 356 80C360 90 378 100 378 100C378 100 396 90 400 80C406 66 390 56 378 70Z"/>"##;
const SPARKLES: &str = r##"<g fill="#FFE48A"><path d="M140 70Q143 84 156 86Q143 88 140 102Q137 88 124 86Q137 84 140 70Z"/><path d="M380 52Q382 62 392 64Q382 66 380 76Q378 66 368 64Q378 62 380 52Z"/></g>"##;
const DROP: &str = r##"<path fill="#9CD5FF" stroke="#5AA0D8" stroke-width="2" d="M346 128Q356 144 350 152Q340 156 338 146Q338 138 346 128Z"/>"##;
/// Her hair behind, her collar, and her face: the icon's paths.
const HEAD: &str = concat!(
    r##"<defs><clipPath id="face"><path d="M178 164C178 116 212 94 256 94C300 94 334 116 334 164C334 202 312 228 284 238C274 242 264 244 256 244C248 244 238 242 228 238C200 228 178 202 178 164Z"/></clipPath></defs>"##,
    r##"<path fill="#EC86B5" d="M256 46C192 46 142 90 136 158C131 214 126 262 118 306L394 306C386 262 381 214 376 158C370 90 320 46 256 46Z"/>"##,
    r##"<path fill="#D9659A" d="M194 200C198 238 214 262 230 276L282 276C298 262 314 238 318 200Z"/>"##,
    r##"<path fill="#F7CDBE" d="M236 222L276 222L276 266C268 270 244 270 236 266Z"/>"##,
    r##"<path fill="#FFFFFF" d="M198 284C204 268 222 258 238 256L256 272L274 256C290 258 308 268 314 284Z"/>"##,
    r##"<path fill="#F05A97" d="M256 264C244 250 226 248 224 258C222 270 236 278 256 264ZM256 264C268 250 286 248 288 258C290 270 276 278 256 264Z"/>"##,
    r##"<circle cx="256" cy="264" r="8" fill="#D94683"/>"##,
    r##"<path fill="#FFE7DC" d="M178 164C178 116 212 94 256 94C300 94 334 116 334 164C334 202 312 228 284 238C274 242 264 244 256 244C248 244 238 242 228 238C200 228 178 202 178 164Z"/>"##,
    r##"<g clip-path="url(#face)"><path fill="#F9D8CB" transform="translate(0 12)" d="M170 176C162 110 204 62 256 62C308 62 350 110 342 176Q328 152 318 134Q316 156 304 172Q298 150 286 130Q286 156 274 178Q268 150 256 128Q244 150 238 178Q226 156 226 130Q214 150 208 172Q196 156 194 134Q184 152 170 176Z"/></g>"##,
);

/// Her side locks, bangs, ahoge and the star in her hair: the icon's paths.
const HAIR: &str = concat!(
    r##"<path fill="#F7A6C9" d="M184 118C164 150 156 204 160 258C162 282 156 300 148 316C164 312 178 300 186 286C190 256 192 214 198 176C200 152 196 132 184 118ZM328 118C348 150 356 204 352 258C350 282 356 300 364 316C348 312 334 300 326 286C322 256 320 214 314 176C312 152 316 132 328 118Z"/>"##,
    r##"<path fill="#F7A6C9" d="M170 176C162 110 204 62 256 62C308 62 350 110 342 176Q328 152 318 134Q316 156 304 172Q298 150 286 130Q286 156 274 178Q268 150 256 128Q244 150 238 178Q226 156 226 130Q214 150 208 172Q196 156 194 134Q184 152 170 176Z"/>"##,
    r##"<path fill="#FFD1E4" d="M196 108C220 84 292 84 316 108C292 92 220 92 196 108Z"/>"##,
    r##"<path fill="#F7A6C9" d="M250 66C238 40 252 18 280 20C264 28 258 44 262 64Z"/>"##,
    r##"<path fill="#FFE48A" d="M318 86Q320 98 332 100Q320 102 318 114Q316 102 304 100Q316 98 318 86Z"/>"##,
);

/// Her fingers over the edge she holds.
const HANDS: &str = concat!(
    r##"<path fill="#FFE7DC" d="M150 286L150 272C150 264 161 264 161 272C161 262 172 262 172 271C172 262 183 262 183 272C183 264 194 264 194 273L194 286ZM318 286L318 273C318 264 329 264 329 272C329 262 340 262 340 271C340 262 351 262 351 272C351 264 362 264 362 272L362 286Z"/>"##,
    r##"<path fill="none" stroke="#F2BBA8" stroke-width="2" stroke-linecap="round" d="M161 274V286M172 273V286M183 274V286M329 274V286M340 273V286M351 274V286"/>"##,
);

#[cfg(test)]
mod tests {
    use resvg::usvg;

    use super::*;

    #[test]
    fn every_face_draws_at_three_times_her_box() {
        for face in Face::ALL {
            let svg = svg(face);
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
                .unwrap_or_else(|error| panic!("{face:?}: {error}"));
            assert_eq!(tree.size().width(), 292.0 * 3.0, "{face:?}");
            assert_eq!(tree.size().height(), 280.0 * 3.0, "{face:?}");
        }
        assert_eq!(images().len(), Face::ALL.len());
    }

    #[test]
    fn each_face_shows_what_it_says() {
        assert!(svg(Face::Surprised).contains(BANG) && svg(Face::Surprised).contains(GASP));
        assert!(svg(Face::Worried).contains(BROWS) && svg(Face::Worried).contains(DROP));
        assert!(svg(Face::Happy).contains(ARCS) && svg(Face::Happy).contains(SPARKLES));
        assert!(svg(Face::Sleepy).contains(LINES) && svg(Face::Sleepy).contains(SNORE));
        assert!(svg(Face::Petted).contains(HEART) && svg(Face::Petted).contains(FLUSH));
        assert!(svg(Face::Blink).contains(SHUT) && !svg(Face::Blink).contains(LEFT_LID));
        assert_ne!(svg(Face::Left), svg(Face::Right));
    }
}
