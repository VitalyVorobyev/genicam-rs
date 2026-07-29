#!/usr/bin/env bash
#
# Fetch a corpus of real-camera GenICam XML descriptions for conformance testing.
#
# The corpus is NOT committed. These documents are vendor copyright, published
# for interoperability by third-party projects; we fetch them on demand rather
# than redistribute them. The download directory is gitignored.
#
# Usage:
#   scripts/fetch-xml-corpus.sh [target-dir]
#
# Then:
#   cargo test -p viva-genapi-xml --test vendor_corpus -- --nocapture
#
# The corpus test is a no-op when the directory is absent, so CI and a fresh
# clone stay green without it.

set -euo pipefail

TARGET="${1:-fixtures/vendor-xml}"

# Real device descriptions collected by the EPICS areaDetector project, covering
# AVT, Basler, Baumer, FLIR, JAI, PCO, Point Grey, Photonic Science, Prosilica,
# SVS, Sony and The Imaging Source. Point Grey / FLIR entries matter most: their
# CDATA-wrapped formulas are what broke issue #45.
AREADETECTOR_REPO="areaDetector/aravisGigE"
AREADETECTOR_PATH="etc/genicam"

# The GenICam standard's own conformance document plus two aravis device
# descriptions. Constant-formula SwissKnives and the `R` access mode live here.
ARAVIS_REPO="AravisProject/aravis"
ARAVIS_FILES=(
  "tests/data/genicam.xml"
  "src/arv-fake-camera.xml"
  "src/arv-v4l2.xml"
)

if ! command -v gh >/dev/null 2>&1; then
  echo "error: this script needs the GitHub CLI (gh) on PATH" >&2
  exit 1
fi

mkdir -p "$TARGET"
echo "Fetching GenICam XML corpus into $TARGET"

echo "  $AREADETECTOR_REPO/$AREADETECTOR_PATH"
gh api "repos/$AREADETECTOR_REPO/contents/$AREADETECTOR_PATH" \
  --jq '.[] | select(.name | endswith(".xml")) | "\(.name)\t\(.download_url)"' |
  while IFS=$'\t' read -r name url; do
    curl -fsSL -o "$TARGET/$name" "$url"
    echo "    $name"
  done

echo "  $ARAVIS_REPO"
for path in "${ARAVIS_FILES[@]}"; do
  name="aravis_$(basename "$path")"
  url="https://raw.githubusercontent.com/$ARAVIS_REPO/main/$path"
  curl -fsSL -o "$TARGET/$name" "$url"
  echo "    $name"
done

# Documents contributed by users on the issue tracker, hosted as GitHub issue
# attachments. Each one is a camera we could not open until it was reported;
# keeping it here is what stops that bug from coming back.
#   name<TAB>url<TAB>issue
USER_CONTRIBUTED=$(
  cat <<'EOF'
Hikrobot_MV-CS050-10GC.xml	https://github.com/user-attachments/files/30513169/xml.raw.xml	35
EOF
)

echo "  user-contributed (issue attachments)"
while IFS=$'\t' read -r name url issue; do
  [ -z "$name" ] && continue
  if curl -fsSL -o "$TARGET/$name" "$url"; then
    echo "    $name (#$issue)"
  else
    echo "    warning: could not fetch $name (#$issue)" >&2
  fi
done <<<"$USER_CONTRIBUTED"

count=$(find "$TARGET" -name '*.xml' | wc -l | tr -d ' ')
echo "Done: $count XML documents in $TARGET"
