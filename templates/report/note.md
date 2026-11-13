{#- Same content as the note on the page; .md = no HTML escaping. -#}
# {{ note.title }}
{%- if !note.shipped.is_empty() %}

## Shipped
{%- for s in note.shipped %}
- {{ s }}
{%- endfor %}
{%- endif %}
{%- if !note.resolved.is_empty() %}

## Resolved in Jira this period
{%- for g in note.resolved %}
{%- if let Some(p) = g.parent %}
- {{ p }}
{%- for i in g.items %}
  - {{ i }}
{%- endfor %}
{%- else %}
{%- for i in g.items %}
- {{ i }}
{%- endfor %}
{%- endif %}
{%- endfor %}
{%- endif %}
{%- for sec in note.sections %}

## {{ sec.heading }}
{%- for i in sec.items %}
- **{{ i.label }}** {{ i.text }}{% if let Some(k) = i.epic %} ({{ k }}){% endif %}{% if let Some(m) = i.meeting %} ({{ m }}){% endif %}{% if let Some(a) = i.via %} (via {{ a }}){% endif %}
{%- endfor %}
{%- endfor %}
{%- if !note.learning.is_empty() %}

## Learning
{%- for l in note.learning %}
- {{ l }}
{%- endfor %}
{%- endif %}
