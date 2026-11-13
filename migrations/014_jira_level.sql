-- 0.2.5 D16: Jira's issuetype.hierarchyLevel (1 = epic level, whatever the team calls it); NULL = not fetched yet,
-- then the type name "Epic" decides.
ALTER TABLE jira_issues ADD COLUMN level INTEGER;
