# Design sources

Canvas: https://claude.ai/artifact/SJY9VrvtZb5r6iZzRzLjSZ (Apple DESIGN.md look + Liquid Glass chrome, Notion UX patterns).

`boards/*.html` are the canvas page bodies. Line 1 = `title|active nav|breadcrumb html`; an optional
`<!--side-->…<!--/side-->` block is extra sidebar content; `@X@` = the hover delete button. `boards/app.css` is the
shared stylesheet every board uses. These are the source for `templates/` and `static/css/`; the boards' sample data
is made up. `.tag` P2/P3/P4 markers are build-order notes, never shipped.
