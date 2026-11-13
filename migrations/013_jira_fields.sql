-- 0.2.5 Epics × Jira: what Atharu reads beyond summary + status. All derived from Jira on each refresh.
ALTER TABLE jira_issues ADD COLUMN issue_type TEXT NOT NULL DEFAULT '';
-- Jira's statusCategory.key: new · indeterminate · done ('' = not fetched since 013). Grouping and Done logic use it,
-- never a status name (each team names its own statuses).
ALTER TABLE jira_issues ADD COLUMN status_cat TEXT NOT NULL DEFAULT '';
ALTER TABLE jira_issues ADD COLUMN parent_key TEXT;
ALTER TABLE jira_issues ADD COLUMN parent_summary TEXT;
-- YYYY-MM-DD.
ALTER TABLE jira_issues ADD COLUMN due TEXT;
ALTER TABLE jira_issues ADD COLUMN resolved_on TEXT;
-- Jira's `updated`, YYYY-MM-DDTHH:MM in Jira's offset.
ALTER TABLE jira_issues ADD COLUMN updated_at TEXT;
-- 1 = returned by the assigned-to-me search on the last refresh (D5).
ALTER TABLE jira_issues ADD COLUMN assigned_me INTEGER NOT NULL DEFAULT 0;
