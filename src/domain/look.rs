//! Looks (spec "Looks", D12/D13): one choice sets the wallpaper, the favicon and, in the Mac app, the Dock icon.
//! Colours come from docs/design/icon/options.html `INKS`; static/css/looks.css and static/icon/<slug>.svg match.

pub struct Look {
    /// `data-look` value, favicon file name, and what `settings.look` stores.
    pub slug: &'static str,
    pub name: &'static str,
    pub jp: &'static str,
    pub blurb: &'static str,
    /// Lowest glass alpha (%) that keeps muted and link text ≥ 4.5:1 on the weakest glass surface (sheet, a − 5)
    /// over this look's darkest veiled wallpaper stop, unblurred (worst case). Measured for T24; see the plan's evidence.
    pub glass_floor: u8,
}

/// Slider default before anyone touches it (D17).
pub const GLASS_DEFAULT: u8 = 60;
/// The Tinted preset's alpha (Apple: "tinted increases opacity").
pub const GLASS_TINTED: u8 = 85;

pub const LOOKS: [Look; 4] = [
    Look {
        slug: "sumi",
        name: "Sumi",
        jp: "墨",
        blurb: "Black ink on plain washi. The default.",
        glass_floor: 18,
    },
    Look {
        slug: "sora",
        name: "Sora",
        jp: "空",
        blurb: "Sky: blue washi with mint and cyan, navy ink.",
        glass_floor: 69,
    },
    Look {
        slug: "wakaba",
        name: "Wakaba",
        jp: "若葉",
        blurb: "Young leaves: green washi with teal and sky blue, pine ink.",
        glass_floor: 57,
    },
    Look {
        slug: "glass",
        name: "Glass",
        jp: "硝子",
        blurb: "Liquid Glass: a deep blue mesh behind clear glass.",
        glass_floor: 83,
    },
];

/// The stored slug if it's a look we ship, else the default (a hand-edited DB can't break a page).
pub fn pick(slug: Option<&str>) -> &'static Look {
    slug.and_then(|s| LOOKS.iter().find(|l| l.slug == s))
        .unwrap_or(&LOOKS[0])
}

/// The alpha (%) every page renders: the stored slider value, never below the look's AA floor (a look switch can
/// raise the floor after the value was saved, so this clamps on read, not on save).
pub fn glass_alpha(look: &Look, stored: Option<&str>) -> u8 {
    stored
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(GLASS_DEFAULT)
        .clamp(look.glass_floor, 100)
}

#[cfg(test)]
mod tests {
    #[test]
    fn glass_alpha_clamps_to_the_looks_floor() {
        let glass = super::pick(Some("glass"));
        assert_eq!(super::glass_alpha(glass, Some("20")), glass.glass_floor);
        assert_eq!(super::glass_alpha(super::pick(None), Some("20")), 20);
        assert_eq!(super::glass_alpha(super::pick(None), Some("250")), 100);
        assert_eq!(
            super::glass_alpha(super::pick(None), Some("300")),
            60,
            "junk → default"
        );
        assert_eq!(super::glass_alpha(super::pick(None), None), 60);
    }

    #[test]
    fn unknown_falls_back_to_sumi() {
        assert_eq!(super::pick(Some("glass")).slug, "glass");
        assert_eq!(super::pick(Some("neon")).slug, "sumi");
        assert_eq!(super::pick(None).slug, "sumi");
    }
}
