{#- Same content as the note on the page; .md = no HTML escaping. -#}
# {{ note.title }}
{%- if !note.shipped.is_empty() %}

## Shipped
{%- for s in note.shipped %}
- {{ s }}
{%- endfor %}
{%- endif %}
{%- for sec in note.sections %}

## {{ sec.heading }}
{%- for i in sec.items %}
- **{{ i.label }}** {{ i.text }}
{%- endfor %}
{%- endfor %}
