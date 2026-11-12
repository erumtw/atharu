use crate::{domain::look::Look, layout::Layout};
use askama::Template;

#[derive(Template)]
#[template(path = "settings/page.html")]
pub struct Page {
    pub layout: Layout,
    pub looks: &'static [Look],
    /// Slider `min`: the current look's AA floor (%).
    pub glass_floor: u8,
}
