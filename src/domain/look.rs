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
    /// Glass alpha (%) while the owner hasn't set one: the light looks share `GLASS_DEFAULT`; Yoru's near-black glass starts at its
    /// floor so the night mesh shows through (the approved board's 38 %), a heavier default would read as a grey slab.
    pub glass_default: u8,
}

/// Slider default before anyone touches it (D17).
pub const GLASS_DEFAULT: u8 = 60;
/// The Tinted preset's alpha (Apple: "tinted increases opacity").
pub const GLASS_TINTED: u8 = 85;

pub const LOOKS: [Look; 6] = [
    Look {
        slug: "sumi",
        name: "Sumi",
        jp: "墨",
        blurb: "Black ink on plain washi. The default.",
        glass_floor: 18,
        glass_default: GLASS_DEFAULT,
    },
    Look {
        slug: "sora",
        name: "Sora",
        jp: "空",
        blurb: "Sky: blue washi with mint and cyan, navy ink.",
        glass_floor: 69,
        glass_default: GLASS_DEFAULT,
    },
    Look {
        slug: "wakaba",
        name: "Wakaba",
        jp: "若葉",
        blurb: "Young leaves: green washi with teal and sky blue, pine ink.",
        glass_floor: 57,
        glass_default: GLASS_DEFAULT,
    },
    Look {
        slug: "glass",
        name: "Glass",
        jp: "硝子",
        blurb: "Liquid Glass: a deep blue mesh behind clear glass.",
        glass_floor: 83,
        glass_default: GLASS_DEFAULT,
    },
    Look {
        slug: "yoru",
        name: "Yoru",
        jp: "夜",
        blurb: "Night: indigo and teal under a dark veil, moonlit ink.",
        // Light text on near-black glass is AA over the whole wallpaper even at 0 % (measured in looks.css); 38 is the board's
        // "clear" setting, the clearest that still shows the mesh without hatching most of the slider.
        glass_floor: 38,
        glass_default: 38,
    },
    Look {
        slug: "auto",
        name: "Auto",
        jp: "自動",
        blurb: "Follows macOS: Sumi by day, Yoru by night.",
        // Both looks' floors must hold: the higher one (Yoru's).
        glass_floor: 38,
        glass_default: 38,
    },
];

/// The stored slug if it's a look we ship, else the default (a hand-edited DB can't break a page).
pub fn pick(slug: Option<&str>) -> &'static Look {
    slug.and_then(|s| LOOKS.iter().find(|l| l.slug == s))
        .unwrap_or(&LOOKS[0])
}

/// The alpha (%) every page renders: the stored slider value as set (owner decision 2026-11-13: the look's AA floor
/// is a marked baseline, not a stop; going clearer is allowed). Junk or nothing stored → the look's default.
pub fn glass_alpha(stored: Option<&str>, look: &Look) -> u8 {
    stored
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(look.glass_default)
        .min(100)
}

/// What plays when the Mac app opens (Settings › Launch animation): `ink` (the default) and `glass` are the two
/// variants of /launch, `off` skips it.
pub const LAUNCH: [&str; 3] = ["ink", "glass", "off"];

/// The stored choice if it's one of `LAUNCH`, else the default (a hand-edited DB can't break the launch).
pub fn launch(stored: Option<&str>) -> &'static str {
    LAUNCH
        .into_iter()
        .find(|v| Some(*v) == stored)
        .unwrap_or(LAUNCH[0])
}

#[cfg(test)]
mod tests {
    #[test]
    fn glass_alpha_goes_below_the_looks_floor() {
        let sumi = super::pick(Some("sumi"));
        assert_eq!(super::glass_alpha(Some("20"), sumi), 20);
        assert_eq!(super::glass_alpha(Some("0"), sumi), 0);
        assert_eq!(super::glass_alpha(Some("100"), sumi), 100);
        assert_eq!(super::glass_alpha(Some("250"), sumi), 100);
        assert_eq!(super::glass_alpha(Some("x"), sumi), 60, "junk → default");
        assert_eq!(super::glass_alpha(None, sumi), 60);
        // Yoru starts clear enough for the night mesh to show; a stored value still wins.
        let yoru = super::pick(Some("yoru"));
        assert_eq!(super::glass_alpha(None, yoru), 38);
        assert_eq!(super::glass_alpha(Some("70"), yoru), 70);
    }

    #[test]
    fn launch_falls_back_to_ink() {
        assert_eq!(super::launch(Some("glass")), "glass");
        assert_eq!(super::launch(Some("off")), "off");
        assert_eq!(super::launch(Some("spin")), "ink");
        assert_eq!(super::launch(None), "ink");
    }

    #[test]
    fn unknown_falls_back_to_sumi() {
        assert_eq!(super::pick(Some("glass")).slug, "glass");
        assert_eq!(super::pick(Some("yoru")).slug, "yoru");
        assert_eq!(super::pick(Some("auto")).slug, "auto");
        assert_eq!(super::pick(Some("neon")).slug, "sumi");
        assert_eq!(super::pick(None).slug, "sumi");
    }
}
