#!/bin/bash
# Long-data seed (0.2.7; CLAUDE.md "Long real data"): a FRESH database of fictional "Acme Grocer" work whose text is as long as the
# owner's real data (60+ character series names, 80-90 character titles, Thai, a 45-character tag, 30 assigned Jira tickets with
# long statuses), so a layout that only works on short demo text fails here. Never a copy of data/atharu.db.
#   scripts/long-seed.sh <out.db>              refuses an existing file (it never deletes anything) and the real data/atharu.db
#   scripts/visual-check.sh --db <out.db>      then runs every page at 1180 / 900 / 390 over it
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=${1:?usage: scripts/long-seed.sh <out.db>}
case "$OUT" in *data/atharu.db*) echo "refusing to touch the real database" >&2; exit 2 ;; esac
[ ! -e "$OUT" ] || { echo "$OUT exists: pick a new file" >&2; exit 2; }
export PATH=/opt/homebrew/opt/rustup/bin:$PATH
BIN=target/debug/atharu
[ -x "$BIN" ] || cargo build -q
A() { "$BIN" --db "$OUT" "$@" --as claude >/dev/null; }
d() { case $1 in -*|+*) date -v"$1"d +%Y-%m-%d ;; *) date -v+"$1"d +%Y-%m-%d ;; esac; }

# Epics: 80-90 character titles, two Thai, long OKRs, one very long tag.
LONGTAG=supplier-scorecards-and-quarterly-review-process
A epic OPS-201 --title "Consolidate the supplier price feed and the warehouse stock sync into one nightly pipeline" --okr "O1 · Fresh on arrival across every delivery zone and every customer segment" --tags checkout,$LONGTAG
A epic OPS-202 --title "ปรับปรุงระบบแจ้งเตือนอุณหภูมิตู้เย็นและการติดตามการจัดส่งสินค้าสดให้ลูกค้าร้านอาหาร" --okr "O1 · Fresh on arrival across every delivery zone and every customer segment" --tags logistics
A epic OPS-203 --title "[Data] Migrate three years of customer order history into the new analytics warehouse schema" --okr "O2 · Repeat orders and the reorder flow for restaurants, hotels and caterers" --tags reorder,$LONGTAG
A epic OPS-204 --title "ทบทวนกระบวนการรับสินค้าและตรวจสอบคุณภาพผักผลไม้สดที่คลังสินค้าภาคเหนือ" --okr "O2 · Repeat orders and the reorder flow for restaurants, hotels and caterers" --tags quality
A epic OPS-205 --title "Quarterly supplier scorecards: automate the collection, scoring and the review pack" --tags $LONGTAG
A epic OPS-206 --title "Short epic"

# Lines, long to-dos (120 characters and more), a note with a 100-character title.
L() { A log "$2" --day "$(d "$1")"; }
L -3 "OPS-201 shipped the nightly price feed behind a flag and compared every total with the old path for a week #$LONGTAG"
L -2 "OPS-203 ! keep the three-year history in cold storage and load only the last twelve months into the live warehouse tables"
L -1 "OPS-202 x waiting on the carrier's webhook spec for the refrigerated truck temperature readings in the northern zone"
L 0  "OPS-204 ตรวจรับผักผลไม้สดรอบเช้าแล้วบันทึกอุณหภูมิและน้ำหนักของทุกพาเลทที่คลังสินค้าภาคเหนือ"
L -2 "> reply to the supplier about the price feed delays and ask for the corrected spreadsheet for every affected delivery zone by 2026-11-12"
L -1 "> send the quarterly supplier scorecard review pack to procurement with the three open questions from the warehouse team by today"
L 0  "OPS-201 > check that the nightly pipeline run finished before the first delivery cut-off and page the on-call if it did not by today"
L 0  "> ติดตามผลการตรวจสอบคุณภาพสินค้าและส่งสรุปให้ทีมคลังสินค้าภาคเหนือก่อนเที่ยง by today"
L 3  "> book the cold-chain sensor demo with the vendor and the warehouse leads for the whole of next week by 2026-11-24"
"$BIN" --db "$OUT" note "Notes from the quarterly supplier business review with procurement, the warehouse leads and the carriers for the whole region" --as claude >/dev/null <<'MD'
A note with a long title. #supplier-scorecards-and-quarterly-review-process

- Price feed accuracy, delivery windows and the cold-chain alerts, one section each.
MD

SERIES1=comps-add-on-cs-hub-from-comops-existing-tools-integration
SERIES2=warehouse-weekly-stock-and-temperature-alerts-review-with-carriers
SERIES3=supplier-onboarding-and-price-feed-quarterly-business-review-emea
sqlite3 "$OUT" <<SQL
UPDATE entries SET author = 'me';
UPDATE epics SET author = 'me', start = '$(d -30)', target = '$(d +20)';
INSERT INTO meetings(day, name, started_at, ended_at) VALUES
  ('$(d -7)', '$SERIES1', '$(d -7) 10:00:00', '$(d -7) 10:40:00'),
  ('$(d -4)', '$SERIES2', '$(d -4) 14:00:00', '$(d -4) 14:50:00'),
  ('$(d -2)', '$SERIES3', '$(d -2) 09:00:00', '$(d -2) 09:45:00'),
  ('$(d 0)', '$SERIES1', '$(d 0) 10:00:00', '$(d 0) 10:35:00');
INSERT INTO taggings(tag, item_type, item_id) VALUES ('cshub-integration', 'series', '$SERIES1'), ('carriers', 'series', '$SERIES2');
UPDATE entries SET meeting_id = 1 WHERE text LIKE 'reply to the supplier%';
UPDATE entries SET meeting_id = 2 WHERE text LIKE 'OPS-202 x waiting%';
UPDATE entries SET meeting_id = 4 WHERE text LIKE 'send the quarterly%';
INSERT INTO events(title, starts_at, ends_at) VALUES
  ('$SERIES1', '$(d 0)T10:00:00', '$(d 0)T10:35:00'),
  ('Quarterly business review with the regional procurement and warehouse leads', '$(d 2)T15:00:00', '$(d 2)T16:00:00');
INSERT INTO topics(slug, summary, own_words) VALUES
  ('write-through-cache-invalidation-strategies-for-multi-warehouse-pricing', '', '');
SQL

# Jira cache: tracked epics with long titles and statuses, and 30 assigned tickets (long summaries, long status names, parents).
STATUSES=("Waiting for UAT sign-off|indeterminate" "In Review by the platform team|indeterminate" "Blocked by supplier|indeterminate" "Ready for the next sprint|new" "Backlog|new" "In Progress|indeterminate")
{
  echo "INSERT INTO jira_issues(key, summary, status, status_cat, issue_type, parent_key, parent_summary, due, updated_at, assigned_me) VALUES"
  echo "('OPS-201','Consolidate the supplier price feed and the warehouse stock sync into one nightly pipeline','Waiting for UAT sign-off','indeterminate','Epic',NULL,NULL,'$(d 20)','$(d -1)T09:00',0),"
  echo "('OPS-203','[Data] Migrate three years of customer order history into the new analytics warehouse schema','In Review by the platform team','indeterminate','Epic',NULL,NULL,NULL,'$(d -1)T09:00',0),"
  for i in $(seq 1 30); do
    st=${STATUSES[$((i % 6))]}
    case $((i % 4)) in 0) par="'OPS-90','Repeat orders programme for restaurants, hotels and caterers across every region'";; 1) par="'OPS-95','Price rules and the supplier price feed for the whole warehouse network'";; *) par="NULL,NULL";; esac
    types=(Bug Story Task)
    type=${types[$((i % 3))]}
    sep=","
    if [ "$i" = 30 ]; then sep=";"; fi
    echo "('OPS-3$(printf %02d "$i")','Demo ticket $i: reconcile the supplier invoice lines with the delivery notes for the northern warehouse network','${st%|*}','${st#*|}','$type',$par,'$(d $((i % 12 + 1)))','$(d -$((i % 9)))T08:00',1)$sep"
  done
} | sqlite3 "$OUT"
echo "long seed ready: $OUT ($("$BIN" --db "$OUT" today >/dev/null 2>&1; sqlite3 "$OUT" 'SELECT COUNT(*) FROM jira_issues WHERE assigned_me = 1') assigned tickets)"
